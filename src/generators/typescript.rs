use crate::generators::ir::{CodegenSpec, Field, Page, TypeExpr};
use crate::spec::ApiSpec;
use crate::traits::{GeneratedOutput, LanguageGenerator};
use handlebars::{handlebars_helper, Handlebars};
use serde_json::json;

pub struct TypeScriptGenerator;

impl LanguageGenerator for TypeScriptGenerator {
    fn target(&self) -> &str {
        "typescript"
    }

    fn file_extension(&self) -> &str {
        "ts"
    }

    fn generate(&self, spec: &ApiSpec) -> GeneratedOutput {
        let ir = CodegenSpec::from_api_spec(spec);
        let ctx = TsContext::new(&ir);

        let mut hb = Handlebars::new();
        hb.set_strict_mode(true);

        handlebars_helper!(pascal_case: |s: str| to_pascal_case(s));
        handlebars_helper!(camel_case: |s: str| to_camel_case(s));
        handlebars_helper!(escape: |s: str| s.replace('\\', "\\\\").replace('\'', "\\'"));
        handlebars_helper!(typescript_type: |ty: TypeExpr| ty.typescript_type());

        hb.register_helper("pascal_case", Box::new(pascal_case));
        hb.register_helper("camel_case", Box::new(camel_case));
        hb.register_helper("escape", Box::new(escape));
        hb.register_helper("typescript_type", Box::new(typescript_type));

        hb.register_template_string("package.json", include_str!("typescript/templates/package.json.hbs"))
            .unwrap();
        hb.register_template_string("tsconfig.json", include_str!("typescript/templates/tsconfig.json.hbs"))
            .unwrap();
        hb.register_template_string("types.ts", include_str!("typescript/templates/types.ts.hbs"))
            .unwrap();
        hb.register_template_string("models.ts", include_str!("typescript/templates/models.ts.hbs"))
            .unwrap();
        hb.register_template_string("parser.ts", include_str!("typescript/templates/parser.ts.hbs"))
            .unwrap();
        hb.register_template_string("client.ts", include_str!("typescript/templates/client.ts.hbs"))
            .unwrap();

        let pages: Vec<_> = ir
            .pages
            .iter()
            .map(|(name, page)| {
                let entity = ir.entities.get(&page.entity).cloned().unwrap_or_else(|| crate::generators::ir::Entity { description: None, list_selector: None, fields: Default::default() });
                let fields: Vec<_> = entity
                    .fields
                    .iter()
                    .map(|(fname, field)| {
                        json!({
                            "name": fname,
                            "camel": to_camel_case(fname),
                            "extraction": ts_extraction(field, &to_camel_case(fname)),
                        })
                    })
                    .collect();
                json!({
                    "name": name,
                    "pascal": to_pascal_case(name),
                    "entity": page.entity,
                    "route": ctx.route(page),
                    "requires_auth": page.requires_auth,
                    "list_selector": page.list_selector,
                    "params": ctx.params(page),
                    "params_sig": ctx.params_sig(page),
                    "return_type": ctx.return_type(page),
                    "fields": fields,
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
                            "camel": to_camel_case(fname),
                            "ty": field.ty.typescript_type(),
                        })
                    })
                    .collect();
                json!({
                    "name": name,
                    "pascal": to_pascal_case(name),
                    "fields": fields,
                })
            })
            .collect();

        let types: Vec<_> = ir
            .types
            .iter()
            .map(|(name, def)| {
                json!({
                    "name": name,
                    "typescript": def.typescript,
                })
            })
            .collect();

        let enums: Vec<_> = ir
            .enums
            .iter()
            .map(|(name, def)| {
                json!({
                    "name": name,
                    "pascal": to_pascal_case(name),
                    "values": def.variants.keys().cloned().collect::<Vec<_>>(),
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
            "entity_imports": ir.entities.keys().map(|k| to_pascal_case(k)).collect::<Vec<_>>().join(", "),
            "enum_imports": ir.enums.keys().map(|k| to_pascal_case(k)).collect::<Vec<_>>().join(", "),
            "pages": pages,
            "entities": entities,
            "types": types,
            "enums": enums,
        });

        let files = vec![
            ("package.json".into(), hb.render("package.json", &json_ctx).unwrap()),
            ("tsconfig.json".into(), hb.render("tsconfig.json", &json_ctx).unwrap()),
            ("src/types.ts".into(), hb.render("types.ts", &json_ctx).unwrap()),
            ("src/models.ts".into(), hb.render("models.ts", &json_ctx).unwrap()),
            ("src/parser.ts".into(), hb.render("parser.ts", &json_ctx).unwrap()),
            ("src/client.ts".into(), hb.render("client.ts", &json_ctx).unwrap()),
        ];

        GeneratedOutput { files }
    }
}

struct TsContext {
    package_name: String,
    client_name: String,
}

impl TsContext {
    fn new(spec: &CodegenSpec) -> Self {
        let package_name = spec.name.to_lowercase().replace([' ', '_'], "-");
        let client_name = format!("{}Client", to_pascal_case(&spec.name));
        Self { package_name, client_name }
    }

    fn return_type(&self, page: &Page) -> String {
        if page.list_selector.is_some() {
            format!("{}[]", page.entity)
        } else {
            page.entity.clone()
        }
    }

    fn params_sig(&self, page: &Page) -> String {
        page.params
            .iter()
            .map(|p| format!("{}: number", to_camel_case(p)))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn params(&self, page: &Page) -> Vec<serde_json::Value> {
        page.params
            .iter()
            .map(|p| json!({"name": p, "camel": to_camel_case(p)}))
            .collect()
    }

    fn route(&self, page: &Page) -> String {
        if page.params.is_empty() {
            page.route.clone()
        } else {
            let mut s = page.route_pattern.clone();
            for p in &page.params {
                s = s.replace(&format!("{{{}}}", p), &format!("${{{}}}", to_camel_case(p)));
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

fn to_camel_case(s: &str) -> String {
    let pascal = to_pascal_case(s);
    let mut chars = pascal.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_lowercase().collect::<String>() + chars.as_str(),
    }
}

fn ts_extraction(field: &Field, prop: &str) -> String {
    let raw = if let Some(attr) = &field.attribute {
        if let Some(sel) = &field.selector {
            format!("$el.find('{}').attr('{}')", escape(sel), escape(attr))
        } else {
            format!("$el.attr('{}')", escape(attr))
        }
    } else if let Some(sel) = &field.selector {
        format!("$el.find('{}').text().trim()", escape(sel))
    } else {
        return format!("{}: {}", prop, if is_optional(field) { "undefined" } else { "\"\"" });
    };

    let ty_string = field.ty.typescript_type();
    let inner = unwrap_option(&ty_string);
    let needs_num = is_numeric(inner);
    let needs_bool = inner == "boolean";

    if needs_num {
        if is_optional(field) {
            format!(
                "{}: (() => {{ const v = {}; return v ? Number(v) : undefined; }})()",
                prop, raw
            )
        } else {
            format!("{}: Number({} || '0')", prop, raw)
        }
    } else if needs_bool {
        if is_optional(field) {
            format!(
                "{}: (() => {{ const v = {}; return v ? v !== 'false' : undefined; }})()",
                prop, raw
            )
        } else {
            format!("{}: ({} || 'false') !== 'false'", prop, raw)
        }
    } else if is_optional(field) {
        format!("{}: {} || undefined", prop, raw)
    } else {
        format!("{}: {}", prop, raw)
    }
}

fn is_optional(field: &Field) -> bool {
    matches!(field.ty, TypeExpr::Option(_))
}

fn unwrap_option(ts: &str) -> &str {
    if let Some(stripped) = ts.strip_suffix(" | null") {
        stripped
    } else if let Some(stripped) = ts.strip_suffix(" | undefined") {
        stripped
    } else {
        ts
    }
}

fn is_numeric(ts: &str) -> bool {
    matches!(ts, "number" | "u32" | "i64" | "u64" | "f64" | "decimal")
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\'', "\\'")
}
