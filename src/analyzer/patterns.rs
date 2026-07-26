use regex::Regex;
use std::collections::HashSet;

use super::FieldType;

/// Detect field type from sample values
pub fn infer_type_from_samples(values: &[String]) -> (FieldType, f64) {
    if values.is_empty() {
        return (FieldType::String, 0.3);
    }

    // Price: digits with optional separators and currency symbols
    let price_re = Regex::new(r"^[\d\s.,]+[\s]*[₽$€£¥₹₴₸฿₺₸₦₡₨]$|^[₽$€£¥₹₴₸฿₺₸₦₡₨][\s]*[\d\s.,]+$").unwrap();
    let price_count = values.iter().filter(|v| price_re.is_match(v.trim())).count();
    if price_count as f64 / values.len() as f64 > 0.7 {
        return (FieldType::Price, 0.9);
    }

    // URL
    let url_re = Regex::new(r"^https?://").unwrap();
    let url_count = values.iter().filter(|v| url_re.is_match(v)).count();
    if url_count as f64 / values.len() as f64 > 0.7 {
        return (FieldType::Url, 0.95);
    }

    // Date/timestamp: multiple formats
    let date_re = Regex::new(
        r"^\d{4}[-/]\d{2}[-/]\d{2}|^\d{2}[-/]\d{2}[-/]\d{4}|^\d{2}\.\d{2}\.\d{4}|\d{1,2}\s+(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\s+\d{4}"
    ).unwrap();
    let date_count = values.iter().filter(|v| date_re.is_match(v.trim())).count();
    if date_count as f64 / values.len() as f64 > 0.7 {
        return (FieldType::Timestamp, 0.85);
    }

    // Integer
    let int_re = Regex::new(r"^\d+$").unwrap();
    let int_count = values.iter().filter(|v| int_re.is_match(v.trim())).count();
    if int_count as f64 / values.len() as f64 > 0.9 {
        return (FieldType::U32, 0.8);
    }

    // Float
    let float_re = Regex::new(r"^\d+[.,]\d+$").unwrap();
    let float_count = values.iter().filter(|v| float_re.is_match(v.trim())).count();
    if float_count as f64 / values.len() as f64 > 0.9 {
        return (FieldType::F64, 0.8);
    }

    // Boolean
    let bool_values: Vec<&str> = vec!["true", "false", "1", "0", "yes", "no"];
    let bool_count = values
        .iter()
        .filter(|v| bool_values.contains(&v.trim().to_lowercase().as_str()))
        .count();
    if bool_count as f64 / values.len() as f64 > 0.9 {
        return (FieldType::Bool, 0.85);
    }

    // Enum: few unique values repeated many times
    let unique: HashSet<&String> = values.iter().collect();
    if unique.len() <= 10 && unique.len() < values.len() {
        let unique_vals: Vec<String> = unique.into_iter().cloned().collect();
        return (FieldType::Enum(unique_vals), 0.7);
    }

    (FieldType::String, 0.6)
}

/// Infer field type from data attribute name and values
pub fn infer_type_from_data_attr(attr_name: &str, values: &[String]) -> (FieldType, f64) {
    let name_lower = attr_name.to_lowercase();

    // ID patterns: data-order, data-user-id, data-game-id, etc.
    if name_lower.contains("id")
        || name_lower.contains("key")
        || name_lower.contains("uid")
    {
        let int_re = Regex::new(r"^\d+$").unwrap();
        let all_numeric = values.iter().all(|v| int_re.is_match(v));
        if all_numeric && !values.is_empty() {
            return (FieldType::Id, 0.9);
        }
    }

    // Bool patterns: data-read, data-self, etc.
    if name_lower.contains("read")
        || name_lower.contains("self")
        || name_lower.contains("active")
        || name_lower.contains("enabled")
        || name_lower.contains("hidden")
        || name_lower.contains("visible")
    {
        let bool_values: Vec<&str> = vec!["true", "false", "1", "0", "yes", "no"];
        let bool_count = values
            .iter()
            .filter(|v| bool_values.contains(&v.as_str()))
            .count();
        if bool_count as f64 / values.len() as f64 > 0.8 {
            return (FieldType::Bool, 0.85);
        }
    }

    // Type patterns: data-type, data-mark, data-chat-type, etc.
    if name_lower.contains("type")
        || name_lower.contains("mark")
        || name_lower.contains("status")
        || name_lower.contains("category")
    {
        let unique: HashSet<&String> = values.iter().collect();
        if unique.len() <= 10 && unique.len() < values.len() && unique.len() > 1 {
            let unique_vals: Vec<String> = unique.into_iter().cloned().collect();
            return (FieldType::Enum(unique_vals), 0.7);
        }
    }

    // Size/count patterns: data-lot-size, data-count, etc.
    if name_lower.contains("size")
        || name_lower.contains("count")
        || name_lower.contains("amount")
    {
        let int_re = Regex::new(r"^\d+$").unwrap();
        let all_numeric = values.iter().all(|v| int_re.is_match(v));
        if all_numeric && !values.is_empty() {
            return (FieldType::U32, 0.8);
        }
    }

    // Fall back to generic inference
    infer_type_from_samples(values)
}

/// Infer field type from CSS class context
pub fn infer_type_from_class(class_name: &str, values: &[String]) -> (FieldType, f64) {
    let lower = class_name.to_lowercase();

    if lower.contains("price") || lower.contains("cost") || lower.contains("amount") {
        let price_re = Regex::new(r"[\d\s]+[.,]\d{2}").unwrap();
        let price_count = values.iter().filter(|v| price_re.is_match(v)).count();
        if price_count as f64 / values.len() as f64 > 0.5 {
            return (FieldType::Price, 0.85);
        }
    }

    if lower.contains("date") || lower.contains("time") || lower.contains("timestamp") {
        return (FieldType::Timestamp, 0.7);
    }

    if lower.contains("img") || lower.contains("image") || lower.contains("avatar")
        || lower.contains("thumb") || lower.contains("icon") || lower.contains("photo")
    {
        let url_re = Regex::new(r"^https?://|^/").unwrap();
        let url_count = values.iter().filter(|v| url_re.is_match(v)).count();
        if url_count as f64 / values.len() as f64 > 0.5 {
            return (FieldType::Url, 0.8);
        }
    }

    if lower.contains("link") || lower.contains("href") || lower.contains("url") {
        let url_re = Regex::new(r"^https?://|^/").unwrap();
        let url_count = values.iter().filter(|v| url_re.is_match(v)).count();
        if url_count as f64 / values.len() as f64 > 0.5 {
            return (FieldType::Url, 0.85);
        }
    }

    infer_type_from_samples(values)
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
