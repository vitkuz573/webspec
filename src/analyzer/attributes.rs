use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct DataAttribute {
    pub element_classes: Vec<String>,
    pub attribute_name: String,
    pub value: String,
    pub element_tag: String,
}

/// Extract all data-* attributes from HTML
pub fn extract_data_attributes(html: &str) -> Vec<DataAttribute> {
    let document = scraper::Html::parse_document(html);
    let mut results = Vec::new();

    for el in document
        .root_element()
        .descendants()
        .filter_map(scraper::ElementRef::wrap)
    {
        for (attr, value) in el.value().attrs() {
            if attr.starts_with("data-") {
                let classes: Vec<String> = el
                    .value()
                    .classes()
                    .map(|c| c.to_string())
                    .collect();
                results.push(DataAttribute {
                    element_classes: classes,
                    attribute_name: attr.to_string(),
                    value: value.to_string(),
                    element_tag: el.value().name().to_string(),
                });
            }
        }
    }

    results
}

/// Group data attributes by element tag + class combination
/// Each unique (tag, class_set) combo is a potential entity
pub fn group_by_entity(attrs: &[DataAttribute]) -> HashMap<String, Vec<DataAttribute>> {
    let mut groups: HashMap<String, Vec<DataAttribute>> = HashMap::new();

    for attr in attrs {
        let key = entity_key(&attr.element_tag, &attr.element_classes);
        groups.entry(key).or_default().push(attr.clone());
    }

    groups
}

/// Generate a stable entity key from tag + classes
fn entity_key(tag: &str, classes: &[String]) -> String {
    if classes.is_empty() {
        tag.to_string()
    } else {
        format!("{}.{}", tag, classes.join("."))
    }
}
