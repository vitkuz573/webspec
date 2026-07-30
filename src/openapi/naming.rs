use std::collections::BTreeSet;

pub fn sanitize_pascal_case(title: &str) -> String {
    let mut out = String::new();
    let mut upper_next = true;
    for c in title.chars() {
        if c.is_alphanumeric() {
            if upper_next {
                out.push(c.to_ascii_uppercase());
                upper_next = false;
            } else {
                out.push(c.to_ascii_lowercase());
            }
        } else {
            upper_next = true;
        }
    }
    if out.is_empty() {
        out.push('S');
        out.push('e');
        out.push('r');
        out.push('v');
        out.push('i');
        out.push('c');
        out.push('e');
    }
    out
}

pub fn operation_page_name(
    path: &str,
    method: &str,
    operation_id: Option<&str>,
    used: &mut BTreeSet<String>,
) -> String {
    let base = if let Some(id) = operation_id {
        sanitize_snake_case(id)
    } else {
        let sanitized = path
            .trim_start_matches('/')
            .replace(['/', '{', '}'], "_")
            .replace('-', "_")
            .to_ascii_lowercase()
            .split('_')
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>()
            .join("_");
        format!("{}_{}", method.to_ascii_lowercase(), sanitized)
    };

    if !used.contains(&base) {
        used.insert(base.clone());
        return base;
    }

    let mut n = 2;
    loop {
        let candidate = format!("{}_{}", base, n);
        if !used.contains(&candidate) {
            used.insert(candidate.clone());
            return candidate;
        }
        n += 1;
    }
}

fn sanitize_snake_case(s: &str) -> String {
    s.replace(['-', '.', '/', '{', '}'], "_")
        .to_ascii_lowercase()
        .split('_')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pascal_case() {
        assert_eq!(sanitize_pascal_case("Swagger Petstore"), "SwaggerPetstore");
        assert_eq!(sanitize_pascal_case("petstore"), "Petstore");
    }

    #[test]
    fn operation_id_preferred() {
        let mut used = BTreeSet::new();
        assert_eq!(
            operation_page_name("/pets", "GET", Some("listPets"), &mut used),
            "listpets"
        );
    }

    #[test]
    fn method_path_fallback() {
        let mut used = BTreeSet::new();
        assert_eq!(
            operation_page_name("/pets/{petId}", "GET", None, &mut used),
            "get_pets_petid"
        );
    }

    #[test]
    fn collision_suffix() {
        let mut used = BTreeSet::new();
        used.insert("get_pets".to_string());
        assert_eq!(
            operation_page_name("/pets", "GET", None, &mut used),
            "get_pets_2"
        );
    }
}
