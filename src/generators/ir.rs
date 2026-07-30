use crate::spec::{ApiSpec, AuthDef, DriftDetectionDef, RateLimitsDef};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Language-agnostic intermediate representation for webspec code generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodegenSpec {
    pub name: String,
    pub base_url: String,
    pub types: IndexMap<String, NewtypeDef>,
    pub enums: IndexMap<String, EnumDef>,
    pub entities: IndexMap<String, Entity>,
    pub pages: IndexMap<String, Page>,
    pub auth: Option<Auth>,
    pub rate_limits: RateLimits,
    pub drift_detection: Option<DriftDetection>,
}

impl CodegenSpec {
    /// Build a language-agnostic IR from a parsed `ApiSpec`.
    pub fn from_api_spec(spec: &ApiSpec) -> Self {
        let types = spec
            .types
            .iter()
            .map(|(name, tm)| {
                let def = NewtypeDef {
                    rust: tm.rust.clone().unwrap_or_else(|| "String".into()),
                    typescript: tm.typescript.clone().unwrap_or_else(|| "string".into()),
                    python: tm.python.clone().unwrap_or_else(|| "str".into()),
                    go: tm.go.clone().unwrap_or_else(|| "string".into()),
                    java: tm.java.clone().unwrap_or_else(|| "String".into()),
                };
                (name.clone(), def)
            })
            .collect();

        let enums = spec
            .enums
            .iter()
            .map(|(name, enum_def)| {
                let def = EnumDef {
                    description: enum_def.description.clone(),
                    variants: enum_def.values.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
                };
                (name.clone(), def)
            })
            .collect();

        let entities = spec
            .entities
            .iter()
            .map(|(name, entity_def)| {
                let fields = entity_def
                    .fields
                    .as_ref()
                    .map(|fields| {
                        fields
                            .iter()
                            .map(|(field_name, field_def)| {
                                let ty = parse_type_expr(&field_def.r#type);
                                let field = Field {
                                    name: field_name.clone(),
                                    ty,
                                    selector: field_def.selector.clone(),
                                    attribute: field_def.attribute.clone(),
                                    transform: field_def.transform.clone(),
                                    description: field_def.description.clone(),
                                };
                                (field_name.clone(), field)
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let entity = Entity {
                    description: entity_def.description.clone(),
                    list_selector: entity_def.list_selector.clone(),
                    fields,
                };
                (name.clone(), entity)
            })
            .collect();

        let auth_required_for: Vec<String> = spec
            .auth
            .as_ref()
            .and_then(|a| a.required_for.clone())
            .unwrap_or_default();

        let pages = spec
            .pages
            .iter()
            .map(|(name, page_def)| {
                let url_pattern = page_def.url_pattern.clone().or(page_def.url.clone());
                let params = url_pattern
                    .as_deref()
                    .map(extract_url_params)
                    .unwrap_or_default();
                let requires_auth = auth_required_for.contains(name);
                let route = page_def.url.clone()
                    .or_else(|| url_pattern.clone().filter(|_| params.is_empty()))
                    .unwrap_or_default();
                let route_pattern = url_pattern.clone().unwrap_or_default();
                let page = Page {
                    description: page_def.description.clone(),
                    entity: page_def.entity.clone(),
                    url: page_def.url.clone(),
                    url_pattern: page_def.url_pattern.clone(),
                    route,
                    route_pattern,
                    list_selector: page_def.list_selector.clone(),
                    method: page_def.method.clone().unwrap_or_else(|| "GET".into()),
                    params,
                    requires_auth,
                };
                (name.clone(), page)
            })
            .collect();

        let auth = spec.auth.as_ref().map(Auth::from_def);
        let rate_limits = RateLimits::from_def(spec.rate_limits.as_ref());
        let drift_detection = spec.drift_detection.as_ref().map(DriftDetection::from_def);

        Self {
            name: spec.name.clone(),
            base_url: spec.base_url.clone().unwrap_or_else(|| "https://example.com".into()),
            types,
            enums,
            entities,
            pages,
            auth,
            rate_limits,
            drift_detection,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewtypeDef {
    pub rust: String,
    pub typescript: String,
    pub python: String,
    pub go: String,
    pub java: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumDef {
    pub description: Option<String>,
    pub variants: IndexMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub description: Option<String>,
    pub list_selector: Option<String>,
    pub fields: IndexMap<String, Field>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    pub ty: TypeExpr,
    pub selector: Option<String>,
    pub attribute: Option<String>,
    pub transform: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page {
    pub description: Option<String>,
    pub entity: String,
    pub url: Option<String>,
    pub url_pattern: Option<String>,
    /// Resolved concrete path for parameterless pages.
    pub route: String,
    /// Pattern used for pages with URL parameters.
    pub route_pattern: String,
    pub list_selector: Option<String>,
    pub method: String,
    pub params: Vec<String>,
    pub requires_auth: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Auth {
    Cookie { cookie_name: String, required_for: Vec<String> },
    Header { header_name: String, required_for: Vec<String> },
    Bearer { header_name: String, required_for: Vec<String> },
}

impl Auth {
    fn from_def(def: &AuthDef) -> Self {
        let required_for = def.required_for.clone().unwrap_or_default();
        match def.r#type.as_deref() {
            Some("cookie") => Auth::Cookie {
                cookie_name: def.cookie_name.clone().unwrap_or_else(|| "session".into()),
                required_for,
            },
            Some("bearer") => Auth::Bearer {
                header_name: "Authorization".into(),
                required_for,
            },
            _ => Auth::Header {
                header_name: "X-Api-Key".into(),
                required_for,
            },
        }
    }

    pub fn required_for(&self) -> &[String] {
        match self {
            Auth::Cookie { required_for, .. }
            | Auth::Header { required_for, .. }
            | Auth::Bearer { required_for, .. } => required_for,
        }
    }

    pub fn auth_type(&self) -> &'static str {
        match self {
            Auth::Cookie { .. } => "cookie",
            Auth::Header { .. } => "header",
            Auth::Bearer { .. } => "bearer",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimits {
    pub requests_per_second: f64,
    pub max_retries: u32,
    pub retry_backoff: RetryBackoff,
}

impl RateLimits {
    fn from_def(def: Option<&RateLimitsDef>) -> Self {
        Self {
            requests_per_second: def.and_then(|d| d.requests_per_second).unwrap_or(1.0),
            max_retries: def.and_then(|d| d.max_retries).unwrap_or(0),
            retry_backoff: RetryBackoff::Exponential,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RetryBackoff {
    Exponential,
    Linear,
    Fixed,
}

impl RetryBackoff {
    pub fn as_str(&self) -> &'static str {
        match self {
            RetryBackoff::Exponential => "exponential",
            RetryBackoff::Linear => "linear",
            RetryBackoff::Fixed => "fixed",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftDetection {
    pub enabled: bool,
    pub interval: Option<String>,
    pub critical_selectors: Vec<CriticalSelector>,
    pub test_urls: Vec<TestUrl>,
    pub pages: IndexMap<String, DriftPage>,
}

impl DriftDetection {
    fn from_def(def: &DriftDetectionDef) -> Self {
        let mut critical = Vec::new();
        let mut pages = IndexMap::new();
        if let Some(page_map) = &def.pages {
            for (page_name, page) in page_map {
                let mut selectors = IndexMap::new();
                for (k, v) in &page.selectors {
                    selectors.insert(k.clone(), v.clone());
                    critical.push(CriticalSelector {
                        selector: v.clone(),
                        context: page_name.clone(),
                        description: None,
                    });
                }
                pages.insert(
                    page_name.clone(),
                    DriftPage {
                        url: page.url.clone(),
                        selectors,
                    },
                );
            }
        }
        Self {
            enabled: def.enabled.unwrap_or(false),
            interval: None,
            critical_selectors: critical,
            test_urls: Vec::new(),
            pages,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriticalSelector {
    pub selector: String,
    pub context: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestUrl {
    pub url: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftPage {
    pub url: String,
    pub selectors: IndexMap<String, String>,
}

/// Normalized type expression with helper methods for target-language mapping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeExpr {
    Primitive(PrimitiveType),
    Named(String),
    Option(Box<TypeExpr>),
    Vec(Box<TypeExpr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrimitiveType {
    String,
    Bool,
    U32,
    I64,
    U64,
    F64,
    Decimal,
    Date,
    DateTime,
    Url,
}

impl TypeExpr {
    /// Render the Rust equivalent of this type.
    pub fn rust_type(&self) -> String {
        match self {
            TypeExpr::Primitive(p) => match p {
                PrimitiveType::String | PrimitiveType::Url => "String".into(),
                PrimitiveType::Bool => "bool".into(),
                PrimitiveType::U32 => "u32".into(),
                PrimitiveType::I64 => "i64".into(),
                PrimitiveType::U64 => "u64".into(),
                PrimitiveType::F64 | PrimitiveType::Decimal => "f64".into(),
                PrimitiveType::Date | PrimitiveType::DateTime => "String".into(),
            },
            TypeExpr::Named(n) => n.clone(),
            TypeExpr::Option(inner) => format!("Option<{}>", inner.rust_type()),
            TypeExpr::Vec(inner) => format!("Vec<{}>", inner.rust_type()),
        }
    }

    /// Render the TypeScript equivalent of this type.
    pub fn typescript_type(&self) -> String {
        match self {
            TypeExpr::Primitive(p) => match p {
                PrimitiveType::String | PrimitiveType::Url => "string".into(),
                PrimitiveType::Bool => "boolean".into(),
                PrimitiveType::U32 | PrimitiveType::I64 | PrimitiveType::U64 | PrimitiveType::F64 | PrimitiveType::Decimal => "number".into(),
                PrimitiveType::Date | PrimitiveType::DateTime => "string".into(),
            },
            TypeExpr::Named(n) => n.clone(),
            TypeExpr::Option(inner) => format!("{} | null", inner.typescript_type()),
            TypeExpr::Vec(inner) => {
                let inner_type = inner.typescript_type();
                if inner_type.ends_with(" | null") {
                    format!("({})[] | null", inner_type.trim_end_matches(" | null"))
                } else {
                    format!("{}[]", inner_type)
                }
            }
        }
    }

    /// Render the Python equivalent of this type.
    pub fn python_type(&self) -> String {
        match self {
            TypeExpr::Primitive(p) => match p {
                PrimitiveType::String | PrimitiveType::Url => "str".into(),
                PrimitiveType::Bool => "bool".into(),
                PrimitiveType::U32 | PrimitiveType::I64 | PrimitiveType::U64 => "int".into(),
                PrimitiveType::F64 | PrimitiveType::Decimal => "float".into(),
                PrimitiveType::Date | PrimitiveType::DateTime => "str".into(),
            },
            TypeExpr::Named(n) => n.clone(),
            TypeExpr::Option(inner) => format!("Optional[{}]", inner.python_type()),
            TypeExpr::Vec(inner) => format!("list[{}]", inner.python_type()),
        }
    }

    /// Render the Go equivalent of this type.
    pub fn go_type(&self) -> String {
        match self {
            TypeExpr::Primitive(p) => match p {
                PrimitiveType::String | PrimitiveType::Url => "string".into(),
                PrimitiveType::Bool => "bool".into(),
                PrimitiveType::U32 => "uint32".into(),
                PrimitiveType::I64 => "int64".into(),
                PrimitiveType::U64 => "uint64".into(),
                PrimitiveType::F64 | PrimitiveType::Decimal => "float64".into(),
                PrimitiveType::Date | PrimitiveType::DateTime => "string".into(),
            },
            TypeExpr::Named(n) => n.clone(),
            TypeExpr::Option(inner) => format!("*{}", inner.go_type()),
            TypeExpr::Vec(inner) => format!("[]{}", inner.go_type()),
        }
    }
}

fn parse_type_expr(s: &str) -> TypeExpr {
    let s = s.trim();
    if let Some(inner) = s.strip_prefix("Option<").and_then(|rest| rest.strip_suffix(">")) {
        return TypeExpr::Option(Box::new(parse_type_expr(inner)));
    }
    if let Some(inner) = s.strip_prefix("Vec<").and_then(|rest| rest.strip_suffix(">")) {
        return TypeExpr::Vec(Box::new(parse_type_expr(inner)));
    }
    if let Some(primitive) = parse_primitive(s) {
        return TypeExpr::Primitive(primitive);
    }
    TypeExpr::Named(s.into())
}

fn parse_primitive(s: &str) -> Option<PrimitiveType> {
    match s {
        "string" => Some(PrimitiveType::String),
        "bool" => Some(PrimitiveType::Bool),
        "u32" => Some(PrimitiveType::U32),
        "i64" => Some(PrimitiveType::I64),
        "u64" => Some(PrimitiveType::U64),
        "f64" => Some(PrimitiveType::F64),
        "decimal" => Some(PrimitiveType::Decimal),
        "date" => Some(PrimitiveType::Date),
        "datetime" => Some(PrimitiveType::DateTime),
        "url" => Some(PrimitiveType::Url),
        _ => None,
    }
}

fn extract_url_params(pattern: &str) -> Vec<String> {
    let mut params = Vec::new();
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            let mut param = String::new();
            while let Some(&nc) = chars.peek() {
                if nc == '}' {
                    chars.next();
                    break;
                }
                param.push(nc);
                chars.next();
            }
            params.push(param);
        }
    }
    params
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn minimal_spec() -> ApiSpec {
        ApiSpec {
            version: "1.0.0".into(),
            protocol: "webspec".into(),
            name: "Minimal".into(),
            base_url: Some("https://example.com".into()),
            info: None,
            types: BTreeMap::new(),
            enums: BTreeMap::new(),
            entities: BTreeMap::new(),
            pages: BTreeMap::new(),
            auth: None,
            rate_limits: None,
            drift_detection: None,
        }
    }

    #[test]
    fn primitives_map_to_all_languages() {
        let cases: Vec<(&str, &str, &str, &str, &str)> = vec![
            ("string", "String", "string", "str", "string"),
            ("bool", "bool", "boolean", "bool", "bool"),
            ("u32", "u32", "number", "int", "uint32"),
            ("i64", "i64", "number", "int", "int64"),
            ("u64", "u64", "number", "int", "uint64"),
            ("f64", "f64", "number", "float", "float64"),
        ];
        for (src, rust, ts, py, go) in cases {
            let ty = parse_type_expr(src);
            assert_eq!(ty.rust_type(), rust);
            assert_eq!(ty.typescript_type(), ts);
            assert_eq!(ty.python_type(), py);
            assert_eq!(ty.go_type(), go);
        }
    }

    #[test]
    fn option_and_vec_types_wrap() {
        let opt = parse_type_expr("Option<u32>");
        assert_eq!(opt.rust_type(), "Option<u32>");
        assert_eq!(opt.typescript_type(), "number | null");
        assert_eq!(opt.python_type(), "Optional[int]");
        assert_eq!(opt.go_type(), "*uint32");

        let vec = parse_type_expr("Vec<string>");
        assert_eq!(vec.rust_type(), "Vec<String>");
        assert_eq!(vec.typescript_type(), "string[]");
        assert_eq!(vec.python_type(), "list[str]");
        assert_eq!(vec.go_type(), "[]string");

        let opt_vec = parse_type_expr("Option<Vec<string>>");
        assert_eq!(opt_vec.typescript_type(), "string[] | null");
    }

    #[test]
    fn named_types_preserved() {
        let ty = parse_type_expr("PageTitle");
        assert_eq!(ty.rust_type(), "PageTitle");
        assert_eq!(ty.typescript_type(), "PageTitle");
    }

    #[test]
    fn ir_preserves_spec_metadata() {
        let spec = minimal_spec();
        let ir = CodegenSpec::from_api_spec(&spec);
        assert_eq!(ir.name, "Minimal");
        assert_eq!(ir.base_url, "https://example.com");
    }

    #[test]
    fn ir_extracts_url_params() {
        let mut spec = minimal_spec();
        let mut pages = BTreeMap::new();
        pages.insert(
            "detail".into(),
            crate::spec::PageDef {
                description: None,
                entity: "PageTitle".into(),
                url: None,
                url_pattern: Some("/items/{id}".into()),
                list_selector: None,
                method: None,
            },
        );
        spec.pages = pages;
        let ir = CodegenSpec::from_api_spec(&spec);
        assert_eq!(ir.pages["detail"].params, vec!["id"]);
    }

    #[test]
    fn ir_marks_auth_required_pages() {
        let mut spec = minimal_spec();
        let mut pages = BTreeMap::new();
        pages.insert(
            "orders".into(),
            crate::spec::PageDef {
                description: None,
                entity: "PageTitle".into(),
                url: Some("/orders".into()),
                url_pattern: None,
                list_selector: None,
                method: None,
            },
        );
        pages.insert(
            "home".into(),
            crate::spec::PageDef {
                description: None,
                entity: "PageTitle".into(),
                url: Some("/".into()),
                url_pattern: None,
                list_selector: None,
                method: None,
            },
        );
        spec.pages = pages;
        spec.auth = Some(crate::spec::AuthDef {
            r#type: Some("cookie".into()),
            cookie_name: Some("session".into()),
            header_name: None,
            required_for: Some(vec!["orders".into()]),
        });
        let ir = CodegenSpec::from_api_spec(&spec);
        assert!(ir.pages["orders"].requires_auth);
        assert!(!ir.pages["home"].requires_auth);
    }
}
