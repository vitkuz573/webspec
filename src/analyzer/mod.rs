pub mod html;
pub mod patterns;
pub mod classes;
pub mod attributes;

use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateEntity {
    pub name: String,
    pub list_selector: String,
    pub fields: Vec<CandidateField>,
    pub item_count: usize,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub url: String,
    pub title: String,
    pub entities: Vec<CandidateEntity>,
    pub url_patterns: Vec<UrlPattern>,
    pub pages_crawled: Vec<CrawledPage>,
    pub raw_html_size: usize,
    pub reduced_html_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrlPattern {
    pub pattern: String,
    pub samples: Vec<String>,
    pub parameters: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawledPage {
    pub url: String,
    pub title: String,
    pub entity_count: usize,
}

impl AnalysisResult {
    pub fn to_yaml(&self) -> String {
        serde_yaml::to_string(self).unwrap_or_default()
    }
}

const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

/// Build an HTTP client with browser-like headers
fn build_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap_or_default()
}

/// Analyze a URL with multi-page crawling
pub async fn analyze_url(url: &str, depth: u32, max_pages: usize) -> anyhow::Result<AnalysisResult> {
    let client = build_client();
    let base_url = extract_base_url(url);

    // 1. Fetch main page
    let html = fetch_page(&client, url).await?;
    let raw_size = html.len();
    let clean_html = html::strip_noise(&html);
    let reduced_size = clean_html.len();
    let document = Html::parse_document(&clean_html);

    // 2. Extract title
    let title = extract_page_title(&document);

    // 3. Discover internal links
    let internal_links = discover_internal_links(&document, &base_url);
    eprintln!("  Found {} internal links", internal_links.len());

    // 4. Determine which pages to crawl (unique paths, prioritized)
    let pages_to_crawl = select_pages_to_crawl(&internal_links, url, max_pages.min(15));
    eprintln!("  Will crawl {} pages (depth={})", pages_to_crawl.len(), depth);

    // 5. Crawl additional pages and collect all data attributes + HTML
    let mut all_data_attrs = attributes::extract_data_attributes(&clean_html);
    let mut all_htmls: Vec<(String, String)> = vec![(url.to_string(), clean_html.clone())];
    let mut crawled_pages: Vec<CrawledPage> = Vec::new();
    let mut crawled_urls: std::collections::HashSet<String> = std::collections::HashSet::new();
    crawled_urls.insert(url.to_string());

    let mut current_page_urls = pages_to_crawl.clone();
    let mut next_depth_urls: Vec<String> = Vec::new();

    for current_depth in 0..depth {
        let urls_at_depth = if current_depth == 0 {
            current_page_urls.clone()
        } else {
            std::mem::take(&mut next_depth_urls)
        };

        if urls_at_depth.is_empty() {
            break;
        }

        for page_url in &urls_at_depth {
            if page_url == url || crawled_urls.contains(page_url) {
                continue;
            }
            if crawled_urls.len() >= max_pages {
                break;
            }

            // Rate limiting: 2 seconds delay between requests to avoid 429
            tokio::time::sleep(std::time::Duration::from_millis(2000)).await;

            match fetch_page(&client, page_url).await {
                Ok(page_html) => {
                    crawled_urls.insert(page_url.clone());
                    let clean = html::strip_noise(&page_html);
                    let page_doc = Html::parse_document(&clean);
                    let page_title = extract_page_title(&page_doc);
                    let page_attrs = attributes::extract_data_attributes(&clean);
                    all_data_attrs.extend(page_attrs);
                    all_htmls.push((page_url.clone(), clean.clone()));
                    crawled_pages.push(CrawledPage {
                        url: page_url.clone(),
                        title: page_title,
                        entity_count: 0,
                    });

                    // If not at max depth, discover links for next level
                    if current_depth + 1 < depth {
                        let page_links = discover_internal_links(&page_doc, &base_url);
                        let new_links: Vec<String> = page_links.into_iter()
                            .filter(|link| !crawled_urls.contains(link) && !current_page_urls.contains(link))
                            .collect();
                        next_depth_urls.extend(new_links);
                    }
                }
                Err(e) => {
                    eprintln!("  Failed to fetch {}: {}", page_url, e);
                }
            }
        }

        current_page_urls = next_depth_urls.clone();
        next_depth_urls.clear();
    }

    // 6. Group data attributes by attribute name
    let attr_groups = attributes::group_by_attr_name(&all_data_attrs);

    // 7. Extract URL patterns from all pages
    let mut all_url_patterns = detect_url_patterns(&document);
    for (_, html_content) in &all_htmls[1..] {
        let doc = Html::parse_document(html_content);
        let mut patterns = detect_url_patterns(&doc);
        all_url_patterns.append(&mut patterns);
    }
    // Deduplicate URL patterns
    all_url_patterns.dedup_by(|a, b| a.pattern == b.pattern);

    // 8. Detect entities from data attributes
    let mut entities = detect_entities_from_data_attrs(&attr_groups);

    // 9. Detect entities from repeated sibling patterns (on main page)
    let main_doc = Html::parse_document(&clean_html);
    let sibling_entities = detect_entities_from_siblings(&main_doc);
    for ent in sibling_entities {
        if !entities.iter().any(|e| e.name == ent.name) {
            entities.push(ent);
        }
    }

    // 10. Detect entities from navigation
    let nav_entities = detect_entities_from_nav(&document, &base_url);
    for ent in nav_entities {
        if !entities.iter().any(|e| e.name == ent.name) {
            entities.push(ent);
        }
    }

    // 11. Map URLs to entities
    let url_entities = map_urls_to_entities(&all_url_patterns, &all_data_attrs);
    for ent in url_entities {
        if !entities.iter().any(|e| e.name == ent.name) {
            entities.push(ent);
        }
    }

    // Sort entities by confidence * item_count
    entities.sort_by(|a, b| {
        let score_a = a.confidence * a.item_count as f64;
        let score_b = b.confidence * b.item_count as f64;
        score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(AnalysisResult {
        url: url.to_string(),
        title,
        entities,
        url_patterns: all_url_patterns,
        pages_crawled: crawled_pages,
        raw_html_size: raw_size,
        reduced_html_size: reduced_size,
    })
}

/// Fetch a page and return its HTML
async fn fetch_page(client: &reqwest::Client, url: &str) -> anyhow::Result<String> {
    let resp = client.get(url).send().await?.error_for_status()?;
    Ok(resp.text().await?)
}

/// Extract base URL (scheme + host)
fn extract_base_url(url: &str) -> String {
    if let Ok(parsed) = url::Url::parse(url) {
        format!("{}://{}", parsed.scheme(), parsed.host_str().unwrap_or(""))
    } else {
        // Fallback: extract up to third /
        let parts: Vec<&str> = url.splitn(3, '/').collect();
        if parts.len() >= 3 {
            format!("{}://{}", parts[0].trim_end_matches(':'), parts[1])
        } else {
            url.to_string()
        }
    }
}

/// Discover all internal links (same domain)
fn discover_internal_links(document: &Html, base_url: &str) -> Vec<String> {
    let sel = Selector::parse("a[href]").unwrap();
    let host = base_url.trim_start_matches("https://").trim_start_matches("http://");

    document
        .select(&sel)
        .filter_map(|el| el.value().attr("href"))
        .filter_map(|href| {
            if href.starts_with('/') {
                Some(format!("{}{}", base_url, href))
            } else if href.starts_with("http") && href.contains(host) {
                Some(href.to_string())
            } else {
                None
            }
        })
        .collect()
}

/// Select the most unique pages to crawl
/// Strategy: prioritize different path prefixes and entity-rich URLs
fn select_pages_to_crawl(links: &[String], main_url: &str, max: usize) -> Vec<String> {
    let mut selected: Vec<String> = Vec::new();
    let mut seen_prefixes: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Always include main URL first
    selected.push(main_url.to_string());
    if let Some(prefix) = get_url_prefix(main_url) {
        seen_prefixes.insert(prefix);
    }

    // Score and rank links by priority
    let mut scored_links: Vec<(String, i32)> = links
        .iter()
        .filter(|link| {
            let path = extract_path(link);
            // Skip non-content paths (static assets, anchors, auth endpoints)
            path != "/"
                && !path.is_empty()
                && !path.starts_with("/auth")
                && !path.starts_with("/login")
                && !path.starts_with("/register")
                && !path.starts_with("/signup")
                && !path.ends_with(".js")
                && !path.ends_with(".css")
                && !path.ends_with(".png")
                && !path.ends_with(".jpg")
                && !path.ends_with(".jpeg")
                && !path.ends_with(".ico")
                && !path.ends_with(".gif")
                && !path.ends_with(".svg")
                && !path.ends_with(".woff")
                && !path.ends_with(".woff2")
                && !path.ends_with(".ttf")
        })
        .map(|link| {
            let path = extract_path(link);
            let priority = score_url_priority(&path);
            (link.clone(), priority)
        })
        .collect();

    // Sort by priority (higher = better)
    scored_links.sort_by(|a, b| b.1.cmp(&a.1));

    // Select: one per unique prefix, up to max
    for (link, _priority) in scored_links {
        if selected.len() >= max {
            break;
        }
        let prefix = get_url_prefix(&link).unwrap_or_default();
        if seen_prefixes.contains(&prefix) {
            continue;
        }
        seen_prefixes.insert(prefix);
        selected.push(link);
    }

    selected
}

/// Get URL prefix (first two path segments) for grouping
fn get_url_prefix(url: &str) -> Option<String> {
    let path = extract_path(url);
    let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    if parts.len() >= 2 {
        Some(format!("{}/{}", parts[0], parts[1]))
    } else if parts.len() == 1 && !parts[0].is_empty() {
        Some(parts[0].to_string())
    } else {
        None
    }
}

/// Score URL priority: higher = more likely to contain entities
fn score_url_priority(path: &str) -> i32 {
    let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    let depth = parts.len();

    // Category/listing pages (e.g., /lots/, /games/, /users/) are more valuable
    // than individual item pages (e.g., /lots/123/)
    let has_id_segment = parts.iter().any(|p| {
        p.chars().all(|c| c.is_ascii_digit()) || p.len() > 20
    });

    // Individual item pages (numeric ID in path) - lower priority
    if has_id_segment && depth >= 2 {
        return 20;
    }

    // Category/listing pages - higher priority
    if depth == 2 && !has_id_segment { return 80; }
    if depth == 1 && !parts.is_empty() && parts[0] != "" { return 70; }
    if depth >= 3 && !has_id_segment { return 60; }

    10
}

/// Extract path from URL
fn extract_path(url: &str) -> String {
    if let Ok(parsed) = url::Url::parse(url) {
        let path = parsed.path().to_string();
        let query = parsed.query().map(|q| format!("?{}", q)).unwrap_or_default();
        format!("{}{}", path, query)
    } else {
        url.splitn(2, '?').nth(0).unwrap_or("/").to_string()
    }
}

/// Detect entities from data attribute groups
fn detect_entities_from_data_attrs(
    attr_groups: &HashMap<String, Vec<attributes::DataAttribute>>,
) -> Vec<CandidateEntity> {
    let mut entities = Vec::new();

    for (attr_name, attrs) in attr_groups {
        if attrs.len() < 2 {
            continue;
        }

        // Skip noise attributes
        if is_noise_attr(attr_name) {
            continue;
        }

        // Determine the entity name from the attribute name
        let entity_name = infer_entity_name_from_attr(attr_name);

        // Collect all unique values
        let values: Vec<String> = attrs.iter().map(|a| a.value.clone()).collect();
        let unique_values: Vec<String> = values.iter().cloned().collect::<std::collections::HashSet<_>>().into_iter().collect();

        // Infer field type
        let (field_type, type_confidence) = patterns::infer_type_from_data_attr(attr_name, &values);

        // Determine the most common element selector for this attribute
        let selector = most_common_selector(attrs);

        // Calculate confidence based on:
        // - number of elements with this attribute
        // - value diversity (too few unique = likely ID, too many unique = likely string)
        // - type confidence
        let count_factor = (attrs.len() as f64).min(50.0) / 50.0; // caps at 50 elements
        let _diversity_ratio = unique_values.len() as f64 / values.len() as f64;
        let confidence = (type_confidence * 0.4 + count_factor * 0.4 + 0.2).min(0.95);

        // Build fields: one for the attribute itself, plus infer related fields from siblings
        let mut fields = vec![CandidateField {
            name: attr_name.strip_prefix("data-").unwrap_or(attr_name).to_string(),
            css_selector: selector.clone(),
            attribute: Some(format!("data-{}", attr_name)),
            field_type,
            confidence: type_confidence,
            sample_values: unique_values.into_iter().take(5).collect(),
            description: String::new(),
        }];

        // Try to extract additional fields from the same elements
        let additional = extract_sibling_fields(attrs);
        fields.extend(additional);

        let item_count = attrs.len();

        entities.push(CandidateEntity {
            name: entity_name,
            list_selector: selector,
            fields,
            item_count,
            confidence,
        });
    }

    entities
}

/// Check if an attribute name is likely noise (framework internals, not data)
fn is_noise_attr(name: &str) -> bool {
    let noise = [
        "toggle", "dismiss", "target", "parent", "container",
        "spy", "offset", "slide", "ride", "interval",
        "wrap", "backdrop", "keyboard", "focus", "tabindex",
        "placement", "trigger", "delay", "animation",
        "react", "ng-", "vue", "bind", "on",
    ];
    noise.iter().any(|n| name == *n || name.starts_with(&format!("{}-", n)))
}

/// Infer entity name from data attribute name — fully generic, no domain knowledge
fn infer_entity_name_from_attr(attr_name: &str) -> String {
    let name = attr_name
        .strip_prefix("data-")
        .unwrap_or(attr_name);

    // Try to extract a meaningful stem: data-user-id → user, data-item-id → item
    let stem = name
        .strip_suffix("-id")
        .or_else(|| name.strip_suffix("-key"))
        .or_else(|| name.strip_suffix("-uid"))
        .or_else(|| name.strip_suffix("-type"))
        .or_else(|| name.strip_suffix("-size"))
        .or_else(|| name.strip_suffix("-count"))
        .unwrap_or(name);

    // If the stem itself has hyphens, take the first segment as the root
    let root = stem.split('-').next().unwrap_or(stem);
    let root = root.split('_').next().unwrap_or(root);

    if root.is_empty() || root == "data" {
        return "Entity".to_string();
    }

    capitalize(root)
}

/// Find the most common selector for a group of attributes
fn most_common_selector(attrs: &[attributes::DataAttribute]) -> String {
    use std::collections::HashMap;
    let mut selector_count: HashMap<String, usize> = HashMap::new();

    for attr in attrs {
        let selector = if attr.element_classes.is_empty() {
            attr.element_tag.clone()
        } else {
            format!("{}.{}", attr.element_tag, attr.element_classes.join("."))
        };
        *selector_count.entry(selector).or_insert(0) += 1;
    }

    selector_count
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(sel, _)| sel)
        .unwrap_or_else(|| "div".to_string())
}

/// Extract additional fields from elements that have the data attribute
fn extract_sibling_fields(attrs: &[attributes::DataAttribute]) -> Vec<CandidateField> {
    // Look at the elements and try to find text content, links, images etc.
    // This is a heuristic based on class names
    use std::collections::HashMap;

    let mut class_field_hints: HashMap<String, Vec<String>> = HashMap::new();

    for attr in attrs {
        for class in &attr.element_classes {
            let lower = class.to_lowercase();
            if lower.contains("price") || lower.contains("cost") {
                class_field_hints
                    .entry("price".to_string())
                    .or_default()
                    .push(class.clone());
            } else if lower.contains("name") || lower.contains("title") || lower.contains("text") {
                class_field_hints
                    .entry("name".to_string())
                    .or_default()
                    .push(class.clone());
            } else if lower.contains("server") {
                class_field_hints
                    .entry("server".to_string())
                    .or_default()
                    .push(class.clone());
            } else if lower.contains("desc") {
                class_field_hints
                    .entry("description".to_string())
                    .or_default()
                    .push(class.clone());
            } else if lower.contains("img") || lower.contains("avatar") || lower.contains("icon") {
                class_field_hints
                    .entry("image_url".to_string())
                    .or_default()
                    .push(class.clone());
            }
        }
    }

    let mut fields = Vec::new();
    for (field_name, classes) in class_field_hints {
        if let Some(most_common) = classes.iter()
            .fold(Option::<(&String, usize)>::None, |acc, c| {
                let count = classes.iter().filter(|x| *x == c).count();
                match acc {
                    None => Some((c, count)),
                    Some((_, prev_count)) if count > prev_count => Some((c, count)),
                    _ => acc,
                }
            })
            .map(|(c, _)| c)
        {
            let tag = attrs.first().map(|a| a.element_tag.as_str()).unwrap_or("div");
            fields.push(CandidateField {
                name: field_name,
                css_selector: format!("{}.{}", tag, most_common),
                attribute: None,
                field_type: FieldType::String,
                confidence: 0.5,
                sample_values: Vec::new(),
                description: String::new(),
            });
        }
    }

    fields
}

/// Detect entities from repeated sibling elements (elements with same tag+class appearing many times)
fn detect_entities_from_siblings(document: &Html) -> Vec<CandidateEntity> {
    let mut entities = Vec::new();

    // Find all elements, group by tag+class, count occurrences
    use std::collections::HashMap;
    let mut selector_elements: HashMap<String, Vec<scraper::ElementRef>> = HashMap::new();

    for el in document
        .root_element()
        .descendants()
        .filter_map(scraper::ElementRef::wrap)
    {
        let classes: Vec<&str> = el.value().classes().collect();
        let key = if classes.is_empty() {
            el.value().name().to_string()
        } else {
            format!("{}.{}", el.value().name(), classes.join("."))
        };
        selector_elements.entry(key).or_default().push(el);
    }

    // Score each group
    let mut scored: Vec<(String, Vec<scraper::ElementRef>, f64)> = Vec::new();

    for (selector, elements) in &selector_elements {
        let count = elements.len();
        if count < 3 {
            continue;
        }

        // Skip common layout classes
        if is_layout_class(selector) {
            continue;
        }

        // Score based on: count, content diversity, child complexity
        let score = score_repeated_pattern(elements, count);
        if score > 0.3 {
            scored.push((selector.clone(), elements.clone(), score));
        }
    }

    // Sort by score
    scored.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    // Take top entities (avoid too many)
    for (selector, elements, score) in scored.into_iter().take(20) {
        let count = elements.len();

        // Extract sample text from elements
        let samples: Vec<String> = elements
            .iter()
            .take(5)
            .filter_map(|el| {
                let text: String = el.text().collect::<Vec<&str>>().join(" ").trim().to_string();
                if text.is_empty() { None } else { Some(text) }
            })
            .collect();

        let (field_type, type_conf) = patterns::infer_type_from_samples(&samples);

        // Try to find child elements that could be sub-fields
        let sub_fields = extract_sub_fields(&elements);

        let field_name = infer_field_name_from_selector(&selector);

        let mut fields = vec![CandidateField {
            name: field_name.clone(),
            css_selector: selector.clone(),
            attribute: None,
            field_type,
            confidence: type_conf,
            sample_values: samples,
            description: String::new(),
        }];

        fields.extend(sub_fields);

        let entity_name = format!("{}Entity", capitalize(&field_name));

        entities.push(CandidateEntity {
            name: entity_name,
            list_selector: selector.clone(),
            fields,
            item_count: count,
            confidence: score,
        });
    }

    entities
}

/// Check if a selector is a common layout class (not an entity)
fn is_layout_class(selector: &str) -> bool {
    let lower = selector.to_lowercase();
    // Only skip truly universal layout primitives
    let layout_keywords = [
        "container", "wrapper", "row", "col-", "flex", "grid",
        "hidden", "visible", "clearfix", "spacer", "divider",
    ];
    layout_keywords.iter().any(|kw| lower.contains(kw))
}

/// Score a repeated pattern: higher = more likely to be a real entity
fn score_repeated_pattern(elements: &[scraper::ElementRef], count: usize) -> f64 {
    if elements.is_empty() {
        return 0.0;
    }

    // Factor 1: count (more = better, log scale)
    let count_score = (count as f64).log2() / 10.0; // 1024 elements -> 1.0

    // Factor 2: content diversity (how many unique text values)
    let texts: Vec<String> = elements
        .iter()
        .map(|el| {
            el.text()
                .collect::<Vec<&str>>()
                .join(" ")
                .trim()
                .to_string()
        })
        .collect();
    let unique_texts: std::collections::HashSet<&String> = texts.iter().collect();
    let diversity_score = if texts.is_empty() {
        0.0
    } else {
        (unique_texts.len() as f64 / texts.len() as f64).min(1.0)
    };

    // Factor 3: child element complexity (more children = more structured = better)
    let avg_children: f64 = elements
        .iter()
        .map(|el| {
            el.children()
                .filter(|c| scraper::ElementRef::wrap(*c).is_some())
                .count() as f64
        })
        .sum::<f64>()
        / elements.len() as f64;
    let child_score = (avg_children / 5.0).min(1.0);

    // Factor 4: has data-* attributes (bonus)
    let has_data_attrs = elements.iter().any(|el| {
        el.value()
            .attrs()
            .any(|(k, _)| k.starts_with("data-"))
    });
    let data_bonus = if has_data_attrs { 0.2 } else { 0.0 };

    // Factor 5: has links (bonus - entities often link somewhere)
    let has_links = elements.iter().any(|el| {
        el.value().name() == "a"
            || el.select(&Selector::parse("a").unwrap()).next().is_some()
    });
    let link_bonus = if has_links { 0.1 } else { 0.0 };

    (count_score * 0.3 + diversity_score * 0.25 + child_score * 0.25 + data_bonus + link_bonus).min(0.95)
}

/// Extract sub-fields from repeated elements (e.g., .tc-price, .tc-server inside .tc-item)
fn extract_sub_fields(elements: &[scraper::ElementRef]) -> Vec<CandidateField> {
    use std::collections::HashMap;

    if elements.is_empty() {
        return Vec::new();
    }

    // Look at children of first few elements
    let mut child_class_count: HashMap<String, HashMap<String, usize>> = HashMap::new();

    for el in elements.iter().take(10) {
        for child in el.children().filter_map(scraper::ElementRef::wrap) {
            let classes: Vec<&str> = child.value().classes().collect();
            if classes.is_empty() {
                continue;
            }
            let child_key = format!("{}.{}", child.value().name(), classes.join("."));
            let text: String = child.text().collect::<Vec<&str>>().join(" ").trim().to_string();
            if !text.is_empty() {
                *child_class_count
                    .entry(child_key)
                    .or_default()
                    .entry(text)
                    .or_insert(0) += 1;
            }
        }
    }

    let mut fields = Vec::new();
    for (child_selector, text_values) in child_class_count {
        let total: usize = text_values.values().sum();
        if total < 2 {
            continue;
        }
        let samples: Vec<String> = text_values.into_keys().take(5).collect();
        let (field_type, conf) = patterns::infer_type_from_samples(&samples);
        let field_name = infer_field_name_from_selector(&child_selector);

        fields.push(CandidateField {
            name: field_name,
            css_selector: child_selector,
            attribute: None,
            field_type,
            confidence: conf,
            sample_values: samples,
            description: String::new(),
        });
    }

    fields
}

/// Detect entities from site navigation (nav, menu, sidebar)
fn detect_entities_from_nav(document: &Html, _base_url: &str) -> Vec<CandidateEntity> {
    let mut entities = Vec::new();

    // Look for nav/menu elements
    let nav_selectors = ["nav", ".nav", ".navbar", ".menu", ".sidebar", "[role='navigation']"];

    for nav_sel in &nav_selectors {
        if let Ok(sel) = Selector::parse(nav_sel) {
            for nav in document.select(&sel) {
                let links: Vec<(String, String)> = nav
                    .select(&Selector::parse("a[href]").unwrap())
                    .filter_map(|a| {
                        let href = a.value().attr("href")?.to_string();
                        let text: String = a.text().collect::<Vec<&str>>().join(" ").trim().to_string();
                        if text.is_empty() || href.starts_with('#') || href.starts_with("javascript") {
                            None
                        } else {
                            Some((text, href))
                        }
                    })
                    .collect();

                if links.len() >= 3 {
                    // This nav reveals site structure
                    let fields: Vec<CandidateField> = links
                        .iter()
                        .map(|(text, href)| CandidateField {
                            name: text.clone(),
                            css_selector: format!("{} a[href]", nav_sel),
                            attribute: Some("href".to_string()),
                            field_type: FieldType::Url,
                            confidence: 0.7,
                            sample_values: vec![href.clone()],
                            description: format!("Nav link: {}", text),
                        })
                        .collect();

                    entities.push(CandidateEntity {
                        name: format!("Navigation ({})", nav_sel),
                        list_selector: format!("{} a[href]", nav_sel),
                        fields,
                        item_count: links.len(),
                        confidence: 0.5,
                    });
                }
            }
        }
    }

    entities
}

/// Map URL patterns to entities
fn map_urls_to_entities(
    url_patterns: &[UrlPattern],
    data_attrs: &[attributes::DataAttribute],
) -> Vec<CandidateEntity> {
    let mut entities = Vec::new();

    for pattern in url_patterns {
        // Map URL patterns to entity names
        let entity_name = infer_entity_from_url_pattern(&pattern.pattern);
        if entity_name.is_empty() {
            continue;
        }

        let fields: Vec<CandidateField> = pattern
            .parameters
            .iter()
            .map(|param| {
                // Try to find a data attribute that matches this parameter
                let matching_attr = data_attrs.iter().find(|a| {
                    let attr_lower = a.attribute_name.to_lowercase();
                    let param_lower = param.to_lowercase();
                    attr_lower.contains(&param_lower)
                        || param_lower.contains(&attr_lower.strip_prefix("data-").unwrap_or(""))
                });

                let (field_type, conf) = if let Some(attr) = matching_attr {
                    let values: Vec<String> = data_attrs
                        .iter()
                        .filter(|a| a.attribute_name == attr.attribute_name)
                        .map(|a| a.value.clone())
                        .collect();
                    patterns::infer_type_from_data_attr(
                        &attr.attribute_name.strip_prefix("data-").unwrap_or(&attr.attribute_name),
                        &values,
                    )
                } else {
                    (FieldType::Id, 0.7)
                };

                CandidateField {
                    name: param.clone(),
                    css_selector: String::new(),
                    attribute: None,
                    field_type,
                    confidence: conf,
                    sample_values: Vec::new(),
                    description: format!("URL parameter: {}", param),
                }
            })
            .collect();

        entities.push(CandidateEntity {
            name: entity_name,
            list_selector: String::new(),
            fields,
            item_count: pattern.samples.len(),
            confidence: 0.6,
        });
    }

    entities
}

/// Infer entity name from URL pattern — fully generic
fn infer_entity_from_url_pattern(pattern: &str) -> String {
    let parts: Vec<&str> = pattern.trim_start_matches('/').split('/').collect();
    // Use the first meaningful segment as entity name
    for part in &parts {
        if part.starts_with('{') {
            continue; // skip parameters
        }
        let clean = part.trim_end_matches('s'); // naive singularize
        if !clean.is_empty() && clean.len() > 1 {
            return capitalize(clean);
        }
    }
    String::new()
}

fn infer_field_name_from_selector(selector: &str) -> String {
    let parts: Vec<&str> = selector.split('.').collect();
    if let Some(last_class) = parts.last() {
        if !last_class.is_empty() {
            return last_class.to_string();
        }
    }
    if let Some(tag) = parts.first() {
        return tag.to_string();
    }
    "field".to_string()
}

pub fn capitalize(s: &str) -> String {
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

    if urls.len() < 3 {
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
        if group_urls.len() < 2 {
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
