use crate::generators::ir::{CodegenSpec, Field, TypeExpr};
use crate::spec::ApiSpec;
use crate::traits::{GeneratedOutput, LanguageGenerator};
use handlebars::{handlebars_helper, Handlebars};
use serde_json::json;

pub struct GoGenerator;

impl LanguageGenerator for GoGenerator {
    fn target(&self) -> &str {
        "go"
    }

    fn file_extension(&self) -> &str {
        "go"
    }

    fn generate(&self, spec: &ApiSpec) -> GeneratedOutput {
        let ir = CodegenSpec::from_api_spec(spec);
        let ctx = GoContext::new(&ir);

        let mut hb = Handlebars::new();
        hb.set_strict_mode(true);

        handlebars_helper!(pascal_case: |s: str| to_pascal_case(s));
        handlebars_helper!(snake_case: |s: str| to_snake_case(s));
        handlebars_helper!(escape: |s: str| s.replace('\\', "\\\\").replace('"', "\\\""));
        handlebars_helper!(go_type: |ty: TypeExpr| ty.go_type());

        hb.register_helper("pascal_case", Box::new(pascal_case));
        hb.register_helper("snake_case", Box::new(snake_case));
        hb.register_helper("escape", Box::new(escape));
        hb.register_helper("go_type", Box::new(go_type));

        hb.register_template_string("go.mod", include_str!("go/templates/go.mod.hbs"))
            .unwrap();
        hb.register_template_string("types.go", include_str!("go/templates/types.go.hbs"))
            .unwrap();
        hb.register_template_string("models.go", include_str!("go/templates/models.go.hbs"))
            .unwrap();
        hb.register_template_string("parser.go", include_str!("go/templates/parser.go.hbs"))
            .unwrap();
        hb.register_template_string("client.go", include_str!("go/templates/client.go.hbs"))
            .unwrap();
        hb.register_template_string("errors.go", include_str!("go/templates/errors.go.hbs"))
            .unwrap();

        let types: Vec<_> = ir
            .types
            .iter()
            .map(|(name, def)| {
                json!({
                    "name": name,
                    "go": def.go,
                })
            })
            .collect();

        let enums: Vec<_> = ir
            .enums
            .iter()
            .map(|(name, def)| {
                json!({
                    "name": name,
                    "variants": def.variants.iter().map(|(k, v)| json!({"key": k, "value": v})).collect::<Vec<_>>(),
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
                    "ty": field.ty.go_type(),
                    "go_type": field.ty.go_type(),
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
                    .map(|(_fname, field)| go_extraction(field))
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
                    "zero_value": ctx.zero_value(page),
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
            "module_name": ctx.module_name,
            "client_name": ctx.client_name,
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
            ("go.mod".into(), hb.render("go.mod", &json_ctx).unwrap()),
            ("types.go".into(), hb.render("types.go", &json_ctx).unwrap()),
            ("models.go".into(), hb.render("models.go", &json_ctx).unwrap()),
            ("parser.go".into(), hb.render("parser.go", &json_ctx).unwrap()),
            ("client.go".into(), hb.render("client.go", &json_ctx).unwrap()),
            ("errors.go".into(), hb.render("errors.go", &json_ctx).unwrap()),
        ];

        GeneratedOutput { files }
    }
}

struct GoContext {
    module_name: String,
    client_name: String,
}

impl GoContext {
    fn new(spec: &CodegenSpec) -> Self {
        let module_name = spec.name.to_lowercase().replace([' ', '_'], "-");
        let client_name = format!("{}Client", to_pascal_case(&spec.name));
        Self { module_name, client_name }
    }

    fn return_type(&self, page: &crate::generators::ir::Page) -> String {
        if page.list_selector.is_some() {
            format!("[]{}", page.entity)
        } else {
            format!("*{}", page.entity)
        }
    }

    fn zero_value(&self, _page: &crate::generators::ir::Page) -> String {
        "nil".into()
    }

    fn params_sig(&self, page: &crate::generators::ir::Page) -> String {
        page.params
            .iter()
            .map(|p| format!("{} int64", to_snake_case(p)))
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
                s = s.replace(&format!("{{{}}}", p), "%d");
            }
            format!("fmt.Sprintf(\"{}\", {})", s, page.params.iter().map(|p| to_snake_case(p)).collect::<Vec<_>>().join(", "))
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

fn go_extraction(field: &Field) -> String {
    let prop = to_pascal_case(&field.name);
    let ty_string = field.ty.go_type();
    let inner = unwrap_option(&ty_string);
    let needs_num = is_numeric(inner);
    let needs_bool = inner == "bool";

    let raw = if let Some(attr) = &field.attribute {
        if let Some(sel) = &field.selector {
            format!("attr(selectOne(n, \"{}\"), \"{}\")", escape(sel), escape(attr))
        } else {
            format!("attr(n, \"{}\")", escape(attr))
        }
    } else if let Some(sel) = &field.selector {
        format!("text(selectOne(n, \"{}\"))", escape(sel))
    } else {
        return format!("{}: {}", prop, if is_optional(field) { "nil" } else { "\"\"" });
    };

    if needs_num {
        if is_optional(field) {
            format!("parse{}Ptr({})", capitalize(inner), raw)
        } else {
            format!("parse{}({})", capitalize(inner), raw)
        }
    } else if needs_bool {
        if is_optional(field) {
            format!("parseBoolPtr({})", raw)
        } else {
            format!("parseBool({})", raw)
        }
    } else {
        raw
    }
}

fn is_optional(field: &Field) -> bool {
    matches!(field.ty, TypeExpr::Option(_))
}

fn unwrap_option(go: &str) -> &str {
    go.strip_prefix('*').unwrap_or(go)
}

fn is_numeric(go: &str) -> bool {
    matches!(go, "int" | "int64" | "uint32" | "uint64" | "float64")
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
