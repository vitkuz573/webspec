use crate::generators::ir::{CodegenSpec, Field, TypeExpr};
use std::collections::HashSet;

pub struct RustContext {
    pub crate_name: String,
    pub client_name: String,
    pub error_name: String,
    pub newtype_names: HashSet<String>,
}

impl RustContext {
    pub fn new(spec: &CodegenSpec) -> Self {
        let crate_name = spec.name.to_lowercase().replace([' ', '_'], "-");
        let client_name = format!("{}Client", pascal_case(&spec.name));
        let error_name = format!("{}Error", pascal_case(&spec.name));
        let newtype_names = spec
            .types
            .keys()
            .cloned()
            .collect::<HashSet<_>>();
        Self {
            crate_name,
            client_name,
            error_name,
            newtype_names,
        }
    }

    pub fn is_newtype_type(&self, name: &str) -> bool {
        self.newtype_names.contains(name)
    }
}

fn pascal_case(s: &str) -> String {
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

/// Render extraction code for a single parser field.
pub fn extraction(field: &Field) -> String {
    let name = snake_case(&field.name);
    let ty = &field.ty;

    let extractor = if let Some(attr) = &field.attribute {
        if let Some(sel) = &field.selector {
            format!("extract_attr(&el, \"{}\", \"{}\")", escape(sel), escape(attr))
        } else {
            format!("el.value().attr(\"{}\").map(|s| s.to_string())", escape(attr))
        }
    } else if let Some(sel) = &field.selector {
        format!("extract_text(&el, \"{}\")", escape(sel))
    } else {
        "None".into()
    };

    let conversion = rust_conversion(ty);
    if extractor == "None" && !matches!(ty, TypeExpr::Option(_)) {
        return format!("let {}: {} = Default::default();", name, ty.rust_type());
    }

    let needs_conversion = !conversion.is_empty();
    let is_option = matches!(ty, TypeExpr::Option(_));

    if needs_conversion {
        if is_option {
            format!("let {} = {}.and_then(|s| {});", name, extractor, conversion)
        } else {
            format!(
                "let {} = {}.and_then(|s| {}).unwrap_or_default();",
                name, extractor, conversion
            )
        }
    } else if is_option {
        format!("let {} = {} ;", name, extractor)
    } else {
        format!(
            "let {} = {}.unwrap_or_default();",
            name, extractor
        )
    }
}

fn rust_conversion(ty: &TypeExpr) -> String {
    match ty {
        TypeExpr::Option(inner) => rust_conversion(inner),
        TypeExpr::Primitive(p) => match p {
            crate::generators::ir::PrimitiveType::U32 => "s.parse::<u32>().ok()".into(),
            crate::generators::ir::PrimitiveType::I64 => "s.parse::<i64>().ok()".into(),
            crate::generators::ir::PrimitiveType::U64 => "s.parse::<u64>().ok()".into(),
            crate::generators::ir::PrimitiveType::F64 | crate::generators::ir::PrimitiveType::Decimal => "s.parse::<f64>().ok()".into(),
            crate::generators::ir::PrimitiveType::Bool => "s.parse::<bool>().ok()".into(),
            _ => String::new(),
        },
        TypeExpr::Named(_) => "s.parse().ok()".into(),
        TypeExpr::Vec(_) => String::new(),
    }
}

fn snake_case(s: &str) -> String {
    let mut out = String::new();
    let mut prev_lower = false;
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 && prev_lower {
            out.push('_');
        }
        out.push(c.to_lowercase().next().unwrap_or(c));
        prev_lower = c.is_lowercase();
    }
    out
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
