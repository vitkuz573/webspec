use std::collections::HashSet;

/// Extract URL pattern from a list of URLs
pub fn extract_url_pattern(urls: &[String]) -> Option<String> {
    if urls.len() < 2 {
        return None;
    }

    let parts: Vec<Vec<&str>> = urls
        .iter()
        .map(|u| u.trim_start_matches('/').split('/').collect())
        .collect();

    let max_len = parts.iter().map(|p| p.len()).max()?;
    let mut pattern_parts = Vec::new();

    for i in 0..max_len {
        let values: Vec<&str> = parts.iter().filter_map(|p| p.get(i)).copied().collect();

        let unique: HashSet<&str> = values.iter().copied().collect();

        if unique.len() == 1 {
            pattern_parts.push(values[0].to_string());
        } else if unique.len() > 1 {
            let all_numeric = values.iter().all(|v| v.chars().all(|c| c.is_ascii_digit()));
            if all_numeric {
                pattern_parts.push("{id}".to_string());
            } else {
                pattern_parts.push("{param}".to_string());
            }
        }
    }

    Some(format!("/{}", pattern_parts.join("/")))
}
