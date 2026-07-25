use regex::Regex;
use std::collections::HashSet;

use super::FieldType;

/// Detect field type from sample values
pub fn infer_type_from_samples(values: &[String]) -> (FieldType, f64) {
    if values.is_empty() {
        return (FieldType::String, 0.3);
    }

    let price_re = Regex::new(r"^[\d\s]+[.,]\d{2}\s*[₽$€]?$").unwrap();
    let price_count = values.iter().filter(|v| price_re.is_match(v)).count();
    if price_count as f64 / values.len() as f64 > 0.7 {
        return (FieldType::Price, 0.9);
    }

    let url_re = Regex::new(r"^https?://").unwrap();
    let url_count = values.iter().filter(|v| url_re.is_match(v)).count();
    if url_count as f64 / values.len() as f64 > 0.7 {
        return (FieldType::Url, 0.95);
    }

    let date_re = Regex::new(r"^\d{4}-\d{2}-\d{2}|^\d{2}\.\d{2}\.\d{4}").unwrap();
    let date_count = values.iter().filter(|v| date_re.is_match(v)).count();
    if date_count as f64 / values.len() as f64 > 0.7 {
        return (FieldType::Timestamp, 0.85);
    }

    let int_re = Regex::new(r"^\d+$").unwrap();
    let int_count = values.iter().filter(|v| int_re.is_match(v)).count();
    if int_count as f64 / values.len() as f64 > 0.9 {
        return (FieldType::U32, 0.8);
    }

    let bool_values: Vec<&str> = vec!["true", "false", "1", "0", "yes", "no"];
    let bool_count = values
        .iter()
        .filter(|v| bool_values.contains(&v.as_str()))
        .count();
    if bool_count as f64 / values.len() as f64 > 0.9 {
        return (FieldType::Bool, 0.85);
    }

    let unique: HashSet<&String> = values.iter().collect();
    if unique.len() <= 10 && unique.len() < values.len() {
        let unique_vals: Vec<String> = unique.into_iter().cloned().collect();
        return (FieldType::Enum(unique_vals), 0.7);
    }

    (FieldType::String, 0.6)
}

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
