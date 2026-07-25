pub mod html;
pub mod patterns;
pub mod classes;
pub mod attributes;

use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A candidate field detected in HTML
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateField {
    pub name: String,
    pub css_selector: String,
    pub attribute: Option<String>,
    pub field_type: FieldType,
    pub confidence: f64,
    pub sample_values: Vec<String>,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FieldType {
    String,
    U32,
    F64,
    Bool,
    Url,
    Timestamp,
    Price,
    Enum(Vec<String>),
    Id,
}

/// A detected repeated pattern (entity candidate)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateEntity {
    pub name: String,
    pub list_selector: String,
    pub fields: Vec<CandidateField>,
    pub item_count: usize,
    pub confidence: f64,
}

/// Full analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub url: String,
    pub title: String,
    pub entities: Vec<CandidateEntity>,
    pub url_patterns: Vec<UrlPattern>,
    pub raw_html_size: usize,
    pub reduced_html_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrlPattern {
    pub pattern: String,
    pub samples: Vec<String>,
    pub parameters: Vec<String>,
}

impl AnalysisResult {
    pub fn to_yaml(&self) -> String {
        serde_yaml::to_string(self).unwrap_or_default()
    }
}

/// Analyze a URL and produce candidate spec
pub async fn analyze_url(url: &str) -> anyhow::Result<AnalysisResult> {
    // 1. Fetch HTML
    let html = reqwest::get(url).await?.text().await?;
    let raw_size = html.len();

    // 2. Strip noise
    let clean_html = html::strip_noise(&html);
    let reduced_size = clean_html.len();

    // 3. Parse DOM
    let document = Html::parse_document(&clean_html);

    // 4. Analyze classes
    let class_map = classes::analyze_classes(&clean_html);
    let semantic_classes = classes::find_semantic_classes(&class_map);

    // 5. Extract data attributes
    let data_attrs = attributes::extract_data_attributes(&clean_html);
    let entity_groups = attributes::group_by_entity(&data_attrs);

    // 6. Find repeated patterns (entities)
    let entities = find_repeated_patterns(&document, &semantic_classes, &entity_groups);

    // 7. Extract title
    let title = extract_page_title(&document);

    // 8. Detect URL patterns (from links on page)
    let url_patterns = detect_url_patterns(&document);

    Ok(AnalysisResult {
        url: url.to_string(),
        title,
        entities,
        url_patterns,
        raw_html_size: raw_size,
        reduced_html_size: reduced_size,
    })
}

fn find_repeated_patterns(
    document: &Html,
    _semantic_classes: &[(String, String)],
    entity_groups: &HashMap<String, Vec<attributes::DataAttribute>>,
) -> Vec<CandidateEntity> {
    let mut entities = Vec::new();

    // Strategy 1: Use entity groups from data-* attributes
    for (entity_name, attrs) in entity_groups {
        if attrs.len() < 3 {
            continue;
        }
        let fields: Vec<CandidateField> = attrs
            .iter()
            .map(|a| CandidateField {
                name: a
                    .attribute_name
                    .strip_prefix("data-")
                    .unwrap_or(&a.attribute_name)
                    .to_string(),
                css_selector: format!("{}.{}", a.element_tag, a.element_classes.join(".")),
                attribute: Some(a.attribute_name.clone()),
                field_type: FieldType::String,
                confidence: 0.5,
                sample_values: vec![a.value.clone()],
                description: String::new(),
            })
            .collect();

        entities.push(CandidateEntity {
            name: entity_name.clone(),
            list_selector: String::new(),
            fields,
            item_count: attrs.len(),
            confidence: 0.6,
        });
    }

    // Strategy 2: Find repeated sibling elements by tag+class
    let repeated = find_repeated_siblings(document);
    for (selector, count) in repeated {
        if count < 3 {
            continue;
        }
        let already = entities.iter().any(|e| e.list_selector == selector);
        if already {
            continue;
        }

        let sel = match Selector::parse(&selector) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let samples: Vec<String> = document
            .select(&sel)
            .take(5)
            .filter_map(|el| {
                let text: String = el.text().collect::<Vec<&str>>().join(" ").trim().to_string();
                if text.is_empty() { None } else { Some(text) }
            })
            .collect();

        let (field_type, confidence) = patterns::infer_type_from_samples(&samples);

        let field_name = infer_field_name_from_selector(&selector);
        entities.push(CandidateEntity {
            name: format!("{}Entity", capitalize(&field_name)),
            list_selector: selector.clone(),
            fields: vec![CandidateField {
                name: field_name,
                css_selector: selector.clone(),
                attribute: None,
                field_type,
                confidence,
                sample_values: samples,
                description: String::new(),
            }],
            item_count: count,
            confidence: 0.5 + confidence * 0.3,
        });
    }

    entities
}

fn find_repeated_siblings(document: &Html) -> Vec<(String, usize)> {
    use std::collections::HashMap;

    let mut selector_count: HashMap<String, usize> = HashMap::new();

    // Walk the DOM tree: for each element, group its children by tag+class
    let root = document.root_element();
    count_sibling_groups(root, &mut selector_count);

    let mut result: Vec<(String, usize)> = selector_count.into_iter().collect();
    result.sort_by(|a, b| b.1.cmp(&a.1));
    result
}

fn count_sibling_groups(
    el: scraper::ElementRef,
    selector_count: &mut std::collections::HashMap<String, usize>,
) {
    let mut class_groups: HashMap<String, usize> = HashMap::new();

    for child in el.children() {
        if let Some(child_el) = scraper::ElementRef::wrap(child) {
            let classes: Vec<&str> = child_el.value().classes().collect();
            if !classes.is_empty() {
                let key = format!("{}.{}", child_el.value().name(), classes.join("."));
                *class_groups.entry(key).or_insert(0) += 1;
            }
        }
    }

    for (key, count) in class_groups {
        if count >= 3 {
            *selector_count.entry(key).or_insert(0) += count;
        }
    }

    // Recurse into children
    for child in el.children() {
        if let Some(child_el) = scraper::ElementRef::wrap(child) {
            count_sibling_groups(child_el, selector_count);
        }
    }
}

/// Infer a generic field name from a CSS selector by extracting the last meaningful class.
/// No domain-specific assumptions — just structural extraction.
fn infer_field_name_from_selector(selector: &str) -> String {
    // Take the last class from the selector as the most specific hint
    let parts: Vec<&str> = selector.split('.').collect();
    if let Some(last_class) = parts.last() {
        // Clean up the class name: take the last segment if it has hyphens
        let clean = last_class.trim_start_matches("tc-");
        if !clean.is_empty() && clean != "item" {
            return clean.to_string();
        }
    }
    // Fall back to tag name
    if let Some(tag) = parts.first() {
        return tag.to_string();
    }
    "field".to_string()
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

fn extract_page_title(document: &Html) -> String {
    let sel = Selector::parse("title").unwrap();
    document
        .select(&sel)
        .next()
        .map(|el| el.text().collect::<String>())
        .unwrap_or_default()
}

fn detect_url_patterns(document: &Html) -> Vec<UrlPattern> {
    let sel = Selector::parse("a[href]").unwrap();
    let urls: Vec<String> = document
        .select(&sel)
        .filter_map(|el| el.value().attr("href"))
        .filter(|h| h.starts_with('/'))
        .map(|h| h.to_string())
        .collect();

    if urls.len() < 5 {
        return Vec::new();
    }

    let mut url_groups: HashMap<String, Vec<String>> = HashMap::new();
    for url in &urls {
        let parts: Vec<&str> = url.trim_start_matches('/').split('/').collect();
        if parts.len() >= 2 {
            let prefix = parts[0..2].join("/");
            url_groups.entry(prefix).or_default().push(url.clone());
        }
    }

    let mut patterns = Vec::new();
    for (_prefix, group_urls) in url_groups {
        if group_urls.len() < 3 {
            continue;
        }

        if let Some(pattern) = patterns::extract_url_pattern(&group_urls) {
            let params = extract_params_from_pattern(&pattern);
            patterns.push(UrlPattern {
                pattern,
                samples: group_urls.into_iter().take(5).collect(),
                parameters: params,
            });
        }
    }

    patterns.sort_by(|a, b| b.samples.len().cmp(&a.samples.len()));
    patterns
}

fn extract_params_from_pattern(pattern: &str) -> Vec<String> {
    let mut params = Vec::new();
    for part in pattern.split('/') {
        if part.starts_with('{') && part.ends_with('}') {
            params.push(part[1..part.len() - 1].to_string());
        }
    }
    params
}
