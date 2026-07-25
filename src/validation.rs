use crate::spec::ApiSpec;
use colored::Colorize;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct ValidationError {
    pub path: String,
    pub message: String,
    pub suggestion: Option<String>,
}

#[derive(Debug)]
pub struct ValidationResult {
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationError>,
}

impl ValidationResult {
    fn new() -> Self {
        Self {
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    fn error(&mut self, path: &str, message: &str) {
        self.errors.push(ValidationError {
            path: path.to_string(),
            message: message.to_string(),
            suggestion: None,
        });
    }

    fn error_with_suggestion(&mut self, path: &str, message: &str, suggestion: &str) {
        self.errors.push(ValidationError {
            path: path.to_string(),
            message: message.to_string(),
            suggestion: Some(suggestion.to_string()),
        });
    }

    fn warning(&mut self, path: &str, message: &str) {
        self.warnings.push(ValidationError {
            path: path.to_string(),
            message: message.to_string(),
            suggestion: None,
        });
    }

    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn print_report(&self) {
        for err in &self.errors {
            eprintln!(
                "{} {} {}",
                "error".red().bold(),
                format!("[{}]", err.path).dimmed(),
                err.message.red()
            );
            if let Some(suggestion) = &err.suggestion {
                eprintln!("  {} {}", "hint:".yellow().bold(), suggestion.yellow());
            }
        }
        for warn in &self.warnings {
            eprintln!(
                "{} {} {}",
                "warning".yellow().bold(),
                format!("[{}]", warn.path).dimmed(),
                warn.message.yellow()
            );
        }
        if self.is_valid() {
            if self.warnings.is_empty() {
                println!("{}", "Spec is valid".green().bold());
            } else {
                println!(
                    "{} ({} warnings)",
                    "Spec is valid".green().bold(),
                    self.warnings.len()
                );
            }
        } else {
            eprintln!(
                "\n{} {} errors, {} warnings",
                "Validation failed:".red().bold(),
                self.errors.len().to_string().red().bold(),
                self.warnings.len().to_string().yellow().bold()
            );
        }
    }
}

pub fn validate(spec: &ApiSpec) -> ValidationResult {
    let mut result = ValidationResult::new();

    validate_structure(spec, &mut result);
    validate_types(spec, &mut result);
    validate_entities(spec, &mut result);
    validate_pages(spec, &mut result);
    validate_selectors(spec, &mut result);
    validate_circular_references(spec, &mut result);

    result
}

fn validate_structure(spec: &ApiSpec, result: &mut ValidationResult) {
    if spec.name.is_empty() {
        result.error("spec.name", "Spec name is empty");
    }
    if spec.version.is_empty() {
        result.error("spec.version", "Spec version is empty");
    }
}

fn validate_types(spec: &ApiSpec, result: &mut ValidationResult) {
    for (name, mapping) in &spec.types {
        if mapping.newtype.unwrap_or(false) {
            let has_lang = mapping.rust.is_some()
                || mapping.typescript.is_some()
                || mapping.python.is_some()
                || mapping.go.is_some()
                || mapping.java.is_some();
            if !has_lang {
                result.warning(
                    &format!("types.{}", name),
                    "Newtype has no language-specific mappings defined",
                );
            }
        }
    }
}

fn validate_entities(spec: &ApiSpec, result: &mut ValidationResult) {
    for (name, entity) in &spec.entities {
        match &entity.fields {
            None => {
                result.warning(
                    &format!("entities.{}", name),
                    "Entity has no fields defined",
                );
            }
            Some(fields) => {
                for (field_name, field_def) in fields {
                    let path = format!("entities.{}.{}", name, field_name);
                    validate_field_type(&field_def.r#type, spec, &path, result);
                    validate_css_selector(
                        field_def.selector.as_deref(),
                        &path,
                        result,
                    );
                }
            }
        }
    }
}

fn validate_field_type(type_name: &str, spec: &ApiSpec, path: &str, result: &mut ValidationResult) {
    if type_name.starts_with("Option<") && type_name.ends_with('>') {
        let inner = &type_name[7..type_name.len() - 1];
        validate_field_type(inner, spec, path, result);
        return;
    }

    if type_name.starts_with("Vec<") && type_name.ends_with('>') {
        let inner = &type_name[4..type_name.len() - 1];
        validate_field_type(inner, spec, path, result);
        return;
    }

    if is_builtin_type(type_name) {
        return;
    }

    if spec.enums.contains_key(type_name) || spec.entities.contains_key(type_name) {
        return;
    }

    if spec.types.contains_key(type_name) {
        return;
    }

    result.error_with_suggestion(
        path,
        &format!("Unknown type: `{}`", type_name),
        "Check if the type is defined in spec.types, spec.enums, or spec.entities",
    );
}

fn is_builtin_type(type_name: &str) -> bool {
    matches!(
        type_name,
        "string" | "String" | "f64" | "u32" | "i64" | "u64" | "bool" | "date" | "datetime"
            | "url" | "decimal"
    )
}

fn validate_css_selector(selector: Option<&str>, path: &str, result: &mut ValidationResult) {
    let sel = match selector {
        Some(s) => s,
        None => return,
    };

    if sel.is_empty() {
        result.error(path, "CSS selector is empty");
        return;
    }

    if !is_valid_css_selector(sel) {
        result.error_with_suggestion(
            path,
            &format!("Invalid CSS selector: `{}`", sel),
            "Ensure the selector uses valid CSS syntax (e.g., .class, #id, tag, [attr])",
        );
    }
}

fn is_valid_css_selector(s: &str) -> bool {
    let mut depth = 0;
    let mut chars = s.chars().peekable();

    while let Some(&c) = chars.peek() {
        match c {
            '(' => depth += 1,
            ')' => {
                if depth == 0 {
                    return false;
                }
                depth -= 1;
            }
            '[' => depth += 1,
            ']' => {
                if depth == 0 {
                    return false;
                }
                depth -= 1;
            }
            ' ' | ',' | '>' | '+' | '~' => {}
            _ => {}
        }
        chars.next();
    }

    depth == 0 && !s.chars().all(|c| c.is_whitespace())
}

fn validate_pages(spec: &ApiSpec, result: &mut ValidationResult) {
    for (name, page) in &spec.pages {
        let path = format!("pages.{}", name);

        if !spec.entities.contains_key(&page.entity) {
            result.error_with_suggestion(
                &path,
                &format!("Page references undefined entity: `{}`", page.entity),
                "Ensure the entity is defined in spec.entities",
            );
        }

        validate_css_selector(page.list_selector.as_deref(), &path, result);
    }
}

fn validate_selectors(spec: &ApiSpec, result: &mut ValidationResult) {
    for (entity_name, entity) in &spec.entities {
        if let Some(fields) = &entity.fields {
            for (field_name, field_def) in fields {
                let path = format!("entities.{}.{}", entity_name, field_name);

                if field_def.attribute.is_some() && field_def.selector.is_none() {
                    result.warning(
                        &path,
                        "Field has attribute extraction but no selector — will extract from current element",
                    );
                }
            }
        }
    }
}

fn validate_circular_references(spec: &ApiSpec, result: &mut ValidationResult) {
    let mut visited = HashSet::new();
    let mut stack = HashSet::new();

    for entity_name in spec.entities.keys() {
        if !visited.contains(entity_name) {
            detect_cycle(
                entity_name,
                spec,
                &mut visited,
                &mut stack,
                &format!("entities.{}", entity_name),
                result,
            );
        }
    }
}

fn detect_cycle(
    name: &str,
    spec: &ApiSpec,
    visited: &mut HashSet<String>,
    stack: &mut HashSet<String>,
    path: &str,
    result: &mut ValidationResult,
) {
    visited.insert(name.to_string());
    stack.insert(name.to_string());

    if let Some(entity) = spec.entities.get(name) {
        if let Some(fields) = &entity.fields {
            for (field_name, field_def) in fields {
                let field_type = unwrap_generics(&field_def.r#type);
                if spec.entities.contains_key(field_type) && !visited.contains(field_type) {
                    let child_path = format!("{}.{}", path, field_name);
                    detect_cycle(field_type, spec, visited, stack, &child_path, result);
                } else if spec.entities.contains_key(field_type)
                    && stack.contains(field_type)
                    && field_type != name
                {
                    result.error(
                        &format!("{}.{}", path, field_name),
                        &format!(
                            "Circular reference detected: `{}` -> `{}`",
                            name, field_type
                        ),
                    );
                }
            }
        }
    }

    stack.remove(name);
}

fn unwrap_generics(type_name: &str) -> &str {
    if type_name.starts_with("Option<") && type_name.ends_with('>') {
        return &type_name[7..type_name.len() - 1];
    }
    if type_name.starts_with("Vec<") && type_name.ends_with('>') {
        return &type_name[4..type_name.len() - 1];
    }
    type_name
}
