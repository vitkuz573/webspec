use std::collections::HashMap;

/// Analyze CSS class frequency across elements
pub fn analyze_classes(html: &str) -> HashMap<String, usize> {
    let document = scraper::Html::parse_document(html);
    let mut class_count: HashMap<String, usize> = HashMap::new();

    for el in document
        .root_element()
        .descendants()
        .filter_map(scraper::ElementRef::wrap)
    {
        for class in el.value().classes() {
            *class_count.entry(class.to_string()).or_insert(0) += 1;
        }
    }

    class_count
}

/// Find classes that hint at a generic field type based on structural patterns.
/// Returns (class_name, type_hint) where type_hint is a structural descriptor,
/// NOT an entity-specific name like "offer" or "game".
pub fn find_semantic_classes(classes: &HashMap<String, usize>) -> Vec<(String, String)> {
    let mut results = Vec::new();

    for (class_name, _count) in classes {
        let lower = class_name.to_lowercase();

        // Classes ending with -id, -key, -uid, etc. → identifier
        if lower.ends_with("-id")
            || lower.ends_with("-key")
            || lower.ends_with("-uid")
            || lower.ends_with("_id")
            || lower.ends_with("_key")
            || lower == "id"
        {
            results.push((class_name.clone(), "id".to_string()));
            continue;
        }

        // Classes containing "data-" or "attr-" prefix in their parts
        if lower.starts_with("data-") || lower.starts_with("attr-") {
            results.push((class_name.clone(), "data-attribute".to_string()));
            continue;
        }

        // Classes ending with common structural suffixes
        if lower.ends_with("-count")
            || lower.ends_with("-total")
            || lower.ends_with("-num")
            || lower.ends_with("-qty")
        {
            results.push((class_name.clone(), "count".to_string()));
            continue;
        }

        if lower.ends_with("-list") || lower.ends_with("-items") || lower.ends_with("-grid") {
            results.push((class_name.clone(), "list-container".to_string()));
            continue;
        }

        if lower.ends_with("-item") || lower.ends_with("-row") || lower.ends_with("-entry") {
            results.push((class_name.clone(), "list-item".to_string()));
            continue;
        }

        if lower.ends_with("-img") || lower.ends_with("-image") || lower.ends_with("-thumb")
            || lower.ends_with("-avatar") || lower.ends_with("-icon") || lower.ends_with("-photo")
        {
            results.push((class_name.clone(), "image".to_string()));
            continue;
        }

        if lower.ends_with("-link") || lower.ends_with("-href") || lower.ends_with("-url") {
            results.push((class_name.clone(), "url".to_string()));
            continue;
        }

        if lower.ends_with("-date") || lower.ends_with("-time") || lower.ends_with("-timestamp")
            || lower.ends_with("-created") || lower.ends_with("-updated")
        {
            results.push((class_name.clone(), "timestamp".to_string()));
            continue;
        }

        if lower.ends_with("-price") || lower.ends_with("-cost") || lower.ends_with("-amount")
            || lower.ends_with("-total")
        {
            results.push((class_name.clone(), "numeric".to_string()));
            continue;
        }
    }

    results
}
