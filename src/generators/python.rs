use crate::generators::ir::{CodegenSpec, Field, TypeExpr};
use crate::spec::ApiSpec;
use crate::traits::{GeneratedOutput, LanguageGenerator};
use handlebars::{handlebars_helper, Handlebars};
use serde_json::json;

pub struct PythonGenerator;

impl LanguageGenerator for PythonGenerator {
    fn target(&self) -> &str {
        "python"
    }

    fn file_extension(&self) -> &str {
        "py"
    }

    fn generate(&self, spec: &ApiSpec) -> GeneratedOutput {
        let ir = CodegenSpec::from_api_spec(spec);
        let ctx = PyContext::new(&ir);

        let mut hb = Handlebars::new();
        hb.set_strict_mode(true);

        handlebars_helper!(pascal_case: |s: str| to_pascal_case(s));
        handlebars_helper!(snake_case: |s: str| to_snake_case(s));
        handlebars_helper!(escape: |s: str| s.replace('\\', "\\\\").replace('"', "\\\""));
        handlebars_helper!(python_type: |ty: TypeExpr| ty.python_type());

        hb.register_helper("pascal_case", Box::new(pascal_case));
        hb.register_helper("snake_case", Box::new(snake_case));
        hb.register_helper("escape", Box::new(escape));
        hb.register_helper("python_type", Box::new(python_type));

        hb.register_template_string(
            "pyproject.toml",
            include_str!("python/templates/pyproject.toml.hbs"),
        )
        .unwrap();
        hb.register_template_string("__init__.py", include_str!("python/templates/__init__.py.hbs"))
            .unwrap();
        hb.register_template_string("types.py", include_str!("python/templates/types.py.hbs"))
            .unwrap();
        hb.register_template_string("models.py", include_str!("python/templates/models.py.hbs"))
            .unwrap();
        hb.register_template_string("parser.py", include_str!("python/templates/parser.py.hbs"))
            .unwrap();
        hb.register_template_string("client.py", include_str!("python/templates/client.py.hbs"))
            .unwrap();

        let types: Vec<_> = ir
            .types
            .iter()
            .map(|(name, def)| {
                json!({
                    "name": name,
                    "python": def.python,
                })
            })
            .collect();

        let enums: Vec<_> = ir
            .enums
            .iter()
            .map(|(name, def)| {
                json!({
                    "name": name,
                    "variants": def.variants.iter().map(|(k, v)| json!({"key": to_snake_case(k), "value": v})).collect::<Vec<_>>(),
                })
            })
            .collect();

        let entities: Vec<_> = ir
            .entities
            .iter()
            .map(|(name, entity)| {
                let fields: Vec<_> = entity
                    .fields
                    .iter()
                    .map(|(fname, field)| {
                        json!({
                            "name": fname,
                            "type": field.ty.python_type(),
                        })
                    })
                    .collect();
                json!({
                    "name": name,
                    "fields": fields,
                })
            })
            .collect();

        let pages: Vec<_> = ir
            .pages
            .iter()
            .map(|(name, page)| {
                let entity = ir.entities.get(&page.entity).cloned().unwrap_or_else(|| crate::generators::ir::Entity {
                    description: None,
                    list_selector: None,
                    fields: Default::default(),
                });
                let fields: Vec<_> = entity
                    .fields
                    .iter()
                    .map(|(_fname, field)| py_extraction(field))
                    .collect();
                json!({
                    "name": name,
                    "entity": page.entity,
                    "return_type": ctx.return_type(page),
                    "list_selector": page.list_selector,
                    "fields": fields,
                    "route": ctx.route(page),
                    "requires_auth": page.requires_auth,
                    "params": ctx.params(page),
                    "params_sig": ctx.params_sig(page),
                })
            })
            .collect();

        let json_ctx = json!({
            "spec": {
                "name": ir.name,
                "base_url": ir.base_url,
                "rate_limits": {
                    "requests_per_second": ir.rate_limits.requests_per_second,
                    "max_retries": ir.rate_limits.max_retries,
                    "retry_backoff": ir.rate_limits.retry_backoff.as_str(),
                },
            },
            "package_name": ctx.package_name,
            "client_name": ctx.client_name,
            "entity_names": ir.entities.keys().map(|k| to_pascal_case(k)).collect::<Vec<_>>(),
            "types": types,
            "enums": enums,
            "entities": entities,
            "pages": pages,
            "auth": ir.auth.as_ref().map(|a| {
                let (ty, key) = match a {
                    crate::generators::ir::Auth::Cookie { cookie_name, .. } => ("cookie", cookie_name.clone()),
                    crate::generators::ir::Auth::Header { header_name, .. } => ("header", header_name.clone()),
                    crate::generators::ir::Auth::Bearer { header_name, .. } => ("header", header_name.clone()),
                };
                json!({"type": ty, "cookie_name": key, "header_name": key})
            }),
        });

        let files = vec![
            ("pyproject.toml".into(), hb.render("pyproject.toml", &json_ctx).unwrap()),
            ("src/__init__.py".into(), hb.render("__init__.py", &json_ctx).unwrap()),
            ("src/types.py".into(), hb.render("types.py", &json_ctx).unwrap()),
            ("src/models.py".into(), hb.render("models.py", &json_ctx).unwrap()),
            ("src/parser.py".into(), hb.render("parser.py", &json_ctx).unwrap()),
            ("src/client.py".into(), hb.render("client.py", &json_ctx).unwrap()),
        ];

        GeneratedOutput { files }
    }
}

struct PyContext {
    package_name: String,
    client_name: String,
}

impl PyContext {
    fn new(spec: &CodegenSpec) -> Self {
        let package_name = spec.name.to_lowercase().replace([' ', '_'], "-").replace('-', "_");
        let client_name = format!("{}Client", to_pascal_case(&spec.name));
        Self { package_name, client_name }
    }

    fn return_type(&self, page: &crate::generators::ir::Page) -> String {
        if page.list_selector.is_some() {
            format!("list[{}]", page.entity)
        } else {
            format!("{} | None", page.entity)
        }
    }

    fn params_sig(&self, page: &crate::generators::ir::Page) -> String {
        page.params
            .iter()
            .map(|p| format!("{}: int", to_snake_case(p)))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn params(&self, page: &crate::generators::ir::Page) -> Vec<serde_json::Value> {
        page.params
            .iter()
            .map(|p| json!({"name": p, "snake": to_snake_case(p)}))
            .collect()
    }

    fn route(&self, page: &crate::generators::ir::Page) -> String {
        if page.params.is_empty() {
            page.route.clone()
        } else {
            let mut s = page.route_pattern.clone();
            for p in &page.params {
                s = s.replace(&format!("{{{}}}", p), &format!("{{{}}}", to_snake_case(p)));
            }
            s
        }
    }
}

fn to_pascal_case(s: &str) -> String {
    s.split(['_', '-'])
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect()
}

fn to_snake_case(s: &str) -> String {
    let pascal = to_pascal_case(s);
    let mut out = String::new();
    let mut prev_lower = false;
    for (i, c) in pascal.chars().enumerate() {
        if c.is_uppercase() && i > 0 && prev_lower {
            out.push('_');
        }
        out.push(c.to_lowercase().next().unwrap_or(c));
        prev_lower = c.is_lowercase();
    }
    out
}

fn py_extraction(field: &Field) -> String {
    let prop = to_snake_case(&field.name);
    let ty_string = field.ty.python_type();
    let inner = unwrap_option(&ty_string);
    let needs_num = is_numeric(inner);
    let needs_bool = inner == "bool";

    let raw = if let Some(attr) = &field.attribute {
        if let Some(sel) = &field.selector {
            format!("el.select_one('{}').get('{}')", escape(sel), escape(attr))
        } else {
            format!("el.get('{}')", escape(attr))
        }
    } else if let Some(sel) = &field.selector {
        format!("el.select_one('{}').text.strip()", escape(sel))
    } else {
        return format!("{}={}", prop, if is_optional(field) { "None" } else { "\"\"" });
    };

    if needs_num {
        if is_optional(field) {
            format!("{}=float({}) if {} is not None else None", prop, raw, raw)
        } else {
            format!("{}=float({}) if {} is not None else 0.0", prop, raw, raw)
        }
    } else if needs_bool {
        if is_optional(field) {
            format!("{}=({}.lower() != 'false') if {} is not None else None", prop, raw, raw)
        } else {
            format!("{}=({}.lower() != 'false') if {} is not None else False", prop, raw, raw)
        }
    } else if is_optional(field) {
        format!("{}={} if {} is not None else None", prop, raw, raw)
    } else {
        format!("{}={} if {} is not None else \"\"", prop, raw, raw)
    }
}

fn is_optional(field: &Field) -> bool {
    matches!(field.ty, TypeExpr::Option(_))
}

fn unwrap_option(py: &str) -> &str {
    py.strip_prefix("Optional[").and_then(|s| s.strip_suffix("]")).unwrap_or(py)
}

fn is_numeric(py: &str) -> bool {
    matches!(py, "int" | "float")
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
