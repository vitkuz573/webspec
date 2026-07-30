use crate::analyzer::{self, RawPageData};
use crate::analyzer::attributes::DataAttribute;
use crate::llm::client::LlmClient;
use crate::llm::prompts;
use crate::llm::ChatMessage;
use crate::spec::{ApiSpec, EntityDef, FieldDef};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;

pub struct DiscoverConfig {
    pub url: String,
    pub api_url: String,
    pub api_key: String,
    pub model: String,
    pub output: Option<PathBuf>,
    pub depth: u32,
    pub max_pages: usize,
    pub no_cache: bool,
}

pub struct DiscoverResult {
    pub raw_data: RawPageData,
    pub spec: ApiSpec,
    pub yaml: String,
}

pub async fn discover(config: DiscoverConfig) -> anyhow::Result<DiscoverResult> {
    eprintln!("Phase 1/2: Static data collection...");
    let raw_data = analyzer::collect_raw_data(&config.url, config.depth, config.max_pages).await?;

    eprintln!("  Found {} selectors, {} data-* attributes, {} URL patterns",
        raw_data.selectors.len(), raw_data.data_attributes.len(), raw_data.url_patterns.len());

    let client = LlmClient::new(&config.api_url, &config.api_key, &config.model)
        .with_cache(!config.no_cache);

    eprintln!("Phase 2/2: LLM spec generation...");
    let yaml_response = llm_generate_spec(&client, &raw_data).await?;

    let spec = ApiSpec::from_str(&yaml_response)?;

    Ok(DiscoverResult {
        raw_data,
        spec,
        yaml: yaml_response,
    })
}

async fn llm_generate_spec(
    client: &LlmClient,
    raw_data: &RawPageData,
) -> anyhow::Result<String> {
    let mut sorted_titles = raw_data.titles.clone();
    sorted_titles.sort();
    let page_titles = sorted_titles.iter()
        .enumerate()
        .map(|(i, t)| format!("{}. {}", i + 1, t))
        .collect::<Vec<_>>()
        .join("\n");

    let html_snippets = build_html_snippets(raw_data);
    let data_attributes = build_data_attributes_str(raw_data);
    let url_patterns = build_url_patterns_str(raw_data);

    let system_prompt = prompts::build_full_spec_prompt(
        &raw_data.url,
        &page_titles,
        &html_snippets,
        &data_attributes,
        &url_patterns,
    );

    let user_msg = "Generate the complete YAML specification now.";

    let messages = vec![
        ChatMessage { role: "system".to_string(), content: system_prompt },
        ChatMessage { role: "user".to_string(), content: user_msg.to_string() },
    ];

    let response = client.chat(messages).await?;

    let yaml = extract_yaml(&response);
    let mut spec = ApiSpec::from_str(&yaml)?;
    post_process(&mut spec);
    auto_add_data_attributes(&mut spec, raw_data);
    dedup_data_attr_fields(&mut spec);
    detect_transforms(&mut spec);

    // Heuristic entity generation for cases where LLM misses entities
    let heuristic_spec = heuristic_entities(raw_data);
    merge_heuristic(&mut spec, heuristic_spec);

    let normalized_yaml = spec.to_yaml()?;
    Ok(normalized_yaml)
}

fn build_html_snippets(raw_data: &RawPageData) -> String {
    let mut sorted_selectors = raw_data.selectors.clone();
    sorted_selectors.sort_by(|a, b| a.selector.cmp(&b.selector));

    let mut snippets = String::new();
    snippets.push_str("VALID INPUT SELECTORS (use ONLY these in your response):\n\n");

    for selector in &sorted_selectors {
        let mut sample_values = selector.sample_values.clone();
        sample_values.sort();
        let samples: Vec<&str> = sample_values.iter().take(3).map(|s| s.as_str()).collect();
        let mut sample_attrs = selector.sample_attributes.clone();
        sample_attrs.sort();
        let attrs: Vec<&str> = sample_attrs.iter().take(3).map(|s| s.as_str()).collect();
        snippets.push_str(&format!(
            "Selector: `{}` (appears {} times)\n  Sample values: {}\n",
            selector.selector,
            selector.count,
            samples.join(" | ")
        ));
        if !attrs.is_empty() {
            snippets.push_str(&format!("  Data attributes: {}\n", attrs.join(" | ")));
        }
        snippets.push('\n');
    }

    snippets
}

fn build_data_attributes_str(raw_data: &RawPageData) -> String {
    if raw_data.data_attributes.is_empty() {
        return "None found.".to_string();
    }

    let mut groups: std::collections::HashMap<String, Vec<&str>> = std::collections::HashMap::new();
    for attr in &raw_data.data_attributes {
        let key = attr.attribute_name.strip_prefix("data-").unwrap_or(&attr.attribute_name);
        groups.entry(key.to_string()).or_default().push(&attr.value);
    }

    let mut sorted_keys: Vec<String> = groups.keys().cloned().collect();
    sorted_keys.sort();

    let mut result = String::new();
    for name in &sorted_keys {
        let values = groups.get(name).unwrap();
        let mut unique: Vec<&str> = values.iter().take(5).copied().collect();
        unique.sort();
        result.push_str(&format!("data-{}: {} occurrences, samples: [{}]\n", name, values.len(), unique.join(", ")));
    }
    result
}

fn build_url_patterns_str(raw_data: &RawPageData) -> String {
    if raw_data.url_patterns.is_empty() {
        return "None detected.".to_string();
    }

    let mut sorted_patterns = raw_data.url_patterns.clone();
    sorted_patterns.sort_by(|a, b| a.pattern.cmp(&b.pattern));

    let mut result = String::new();
    for pattern in &sorted_patterns {
        let mut samples = pattern.samples.clone();
        samples.sort();
        let sample_refs: Vec<&str> = samples.iter().take(3).map(|s| s.as_str()).collect();
        let mut params = pattern.parameters.clone();
        params.sort();
        result.push_str(&format!(
            "Pattern: {} ({} samples)\n  Examples: {}\n  Parameters: {:?}\n\n",
            pattern.pattern, pattern.samples.len(), sample_refs.join(", "), params
        ));
    }
    result
}

fn post_process(spec: &mut ApiSpec) {
    // 1. Remove entities with 0 fields
    spec.entities.retain(|_, e| {
        e.fields.as_ref().map_or(false, |f| !f.is_empty())
    });

    // 2. Normalize attribute values
    for entity in spec.entities.values_mut() {
        if let Some(fields) = &mut entity.fields {
            for field in fields.values_mut() {
                if field.attribute.is_none() {
                    if let Some(sel) = &field.selector {
                        if sel.contains("a.") || sel.starts_with("a ") || sel == "a" {
                            field.attribute = Some("href".to_string());
                        } else if sel.contains("img") {
                            field.attribute = Some("src".to_string());
                        } else {
                            field.attribute = Some("text".to_string());
                        }
                    }
                }
            }
        }
    }

    // 3. Remove unused types
    let mut used_types: HashSet<String> = HashSet::new();
    for entity in spec.entities.values() {
        if let Some(fields) = &entity.fields {
            for field in fields.values() {
                used_types.insert(field.r#type.clone());
            }
        }
    }
    spec.types.retain(|name, _| used_types.contains(name));

    // 4. Clean up empty optional sections
    spec.enums.retain(|_, e| !e.values.is_empty());
    spec.pages.retain(|_, _| true); // pages are always kept if present
    if spec.auth.as_ref().map_or(true, |a| {
        a.r#type.is_none() && a.cookie_name.is_none() && a.required_for.is_none()
    }) {
        spec.auth = None;
    }
    if spec.rate_limits.as_ref().map_or(true, |r| {
        r.requests_per_second.is_none() && r.max_retries.is_none()
    }) {
        spec.rate_limits = None;
    }
    if spec.drift_detection.as_ref().map_or(true, |d| {
        d.enabled.is_none() && d.pages.as_ref().map_or(true, |p| p.is_empty())
    }) {
        spec.drift_detection = None;
    }

    // 5. Filter LLM hallucinations (invalid selectors)
    let valid_html_tags = [
        "a", "abbr", "address", "area", "article", "aside", "audio", "b", "base",
        "bdi", "bdo", "blockquote", "body", "br", "button", "canvas", "caption",
        "cite", "code", "col", "colgroup", "data", "datalist", "dd", "del",
        "details", "dfn", "dialog", "div", "dl", "dt", "em", "embed",
        "fieldset", "figcaption", "figure", "footer", "form", "h1", "h2", "h3",
        "h4", "h5", "h6", "head", "header", "hr", "html", "i", "iframe", "img",
        "input", "ins", "kbd", "label", "legend", "li", "link", "main", "map",
        "mark", "meta", "meter", "nav", "noscript", "object", "ol", "optgroup",
        "option", "output", "p", "param", "picture", "pre", "progress", "q",
        "rp", "rt", "ruby", "s", "samp", "script", "section", "select", "small",
        "source", "span", "strong", "style", "sub", "summary", "sup", "table",
        "tbody", "td", "template", "textarea", "tfoot", "th", "thead", "time",
        "title", "tr", "track", "u", "ul", "var", "video", "wbr",
    ];
    let tag_set: HashSet<&str> = valid_html_tags.iter().copied().collect();
    let css_chars = ['.', '#', '[', ']', ':', '>', '~', '+'];

    for entity in spec.entities.values_mut() {
        if let Some(fields) = &mut entity.fields {
            fields.retain(|_, field| {
                let Some(sel) = &field.selector else {
                    return false; // no selector = hallucination
                };
                let sel = sel.trim();
                if sel.is_empty() {
                    return false;
                }
                // Must start with valid CSS selector character
                let first = sel.chars().next().unwrap();
                if first != '.' && first != '#' && first != ':' && first != '['
                    && first != '>' && !first.is_ascii_alphabetic()
                {
                    return false;
                }
                // Must contain at least one CSS structural character or be a bare tag
                let looks_like_random_text = !sel.chars().any(|c| css_chars.contains(&c))
                    && !tag_set.contains(sel);
                if looks_like_random_text {
                    return false;
                }
                true
            });
        }
    }
}

fn auto_add_data_attributes(spec: &mut ApiSpec, raw_data: &RawPageData) {
    if raw_data.data_attributes.is_empty() {
        return;
    }

    // Group data attributes by element_key (tag.class) for fast lookup
    let mut by_element: HashMap<String, Vec<&DataAttribute>> = HashMap::new();
    for attr in &raw_data.data_attributes {
        let key = if attr.element_classes.is_empty() {
            attr.element_tag.clone()
        } else {
            format!("{}.{}", attr.element_tag, attr.element_classes.join("."))
        };
        by_element.entry(key).or_default().push(attr);
    }

    // For each entity with a list_selector, find matching data-* attributes
    for (_name, entity) in spec.entities.iter_mut() {
        let list_sel = match &entity.list_selector {
            Some(s) => s.clone(),
            None => continue,
        };

        let sel_trimmed = list_sel.trim();
        let mut matching_attrs: Vec<&DataAttribute> = Vec::new();

        // Try direct match: selector like "a.tc-item" matches element key "a.tc-item"
        if let Some(attrs) = by_element.get(sel_trimmed) {
            matching_attrs.extend(attrs);
        }

        // Also try tag-only match and class-only match
        for (element_key, attrs) in &by_element {
            if element_key == sel_trimmed {
                continue; // already matched
            }
            // Check if element_key matches the selector's tag or class parts
            let sel_tag = sel_trimmed.split('.').next().unwrap_or("");
            let sel_classes: Vec<&str> = sel_trimmed.split('.').skip(1).collect();
            let elem_tag = element_key.split('.').next().unwrap_or("");
            let elem_classes: Vec<&str> = element_key.split('.').skip(1).collect();

            if !sel_tag.is_empty() && sel_tag == elem_tag {
                // Tag matches — check if any class overlaps
                if sel_classes.iter().any(|sc| elem_classes.contains(sc)) {
                    for attr in attrs {
                        if !matching_attrs.iter().any(|m| {
                            m.attribute_name == attr.attribute_name
                                && m.value == attr.value
                        }) {
                            matching_attrs.push(attr);
                        }
                    }
                }
            }
        }

        // Deduplicate matching attrs by attribute_name only
        let mut seen_attr_names: HashSet<String> = HashSet::new();
        let mut deduped_attrs: Vec<&DataAttribute> = Vec::new();
        for attr in &matching_attrs {
            if seen_attr_names.insert(attr.attribute_name.clone()) {
                deduped_attrs.push(attr);
            }
        }

        // Collect unique attribute names from matching elements
        let mut unique_attrs: HashMap<String, Vec<&DataAttribute>> = HashMap::new();
        for attr in &deduped_attrs {
            unique_attrs
                .entry(attr.attribute_name.clone())
                .or_default()
                .push(attr);
        }

        if unique_attrs.is_empty() {
            continue;
        }

        let fields = entity.fields.get_or_insert_with(BTreeMap::new);

        // Track which data-* attributes are already used by existing fields
        let mut used_data_attrs: HashSet<String> = HashSet::new();
        for (_fn, fdef) in fields.iter() {
            if let Some(attr) = &fdef.attribute {
                if attr.starts_with("data-") {
                    used_data_attrs.insert(attr.clone());
                }
            }
        }

        for (attr_name, attr_samples) in &unique_attrs {
            // Skip if this data-* attribute is already used by an existing field
            if used_data_attrs.contains(attr_name) {
                continue;
            }
            // Derive field name: strip "data-" prefix, replace hyphens with underscores
            let field_name = attr_name
                .strip_prefix("data-")
                .unwrap_or(attr_name)
                .replace('-', "_");

            // Skip if field already exists
            if fields.contains_key(&field_name) {
                continue;
            }

            // Detect type from sample values
            let sample_value = attr_samples.first().map(|a| a.value.as_str()).unwrap_or("");
            let field_type = if looks_numeric(sample_value) {
                "u32".to_string()
            } else if looks_like_id(sample_value) {
                "u32".to_string()
            } else {
                "String".to_string()
            };

            let transform = if field_name.ends_with("_id") && field_type == "u32" {
                Some("parse_id_from_url".to_string())
            } else {
                None
            };

            fields.insert(
                field_name,
                FieldDef {
                    r#type: field_type,
                    nullable: Some(false),
                    selector: Some(list_sel.clone()),
                    attribute: Some(attr_name.clone()),
                    transform,
                    description: None,
                },
            );
        }
    }
}

fn detect_transforms(spec: &mut ApiSpec) {
    for (_name, entity) in spec.entities.iter_mut() {
        let fields = match &mut entity.fields {
            Some(f) => f,
            None => continue,
        };
        for (_fname, field) in fields.iter_mut() {
            if field.transform.is_some() {
                continue;
            }
            let fname = _fname.to_lowercase();
            let typ = field.r#type.as_str();

            if fname.ends_with("_id") && typ == "u32" {
                field.transform = Some("parse_id_from_url".to_string());
            } else if fname.contains("price") && typ == "String" {
                field.transform = Some("parse_price".to_string());
            } else if (fname.ends_with("_date") || fname.ends_with("_time")
                || fname.starts_with("date_") || fname.starts_with("time_"))
                && typ == "String"
            {
                field.transform = Some("parse_date".to_string());
            } else if (fname.ends_with("_count")
                || fname.ends_with("_amount")
                || fname.ends_with("_size"))
                && typ == "String"
            {
                field.transform = Some("parse_number".to_string());
            }
        }
    }
}

fn looks_numeric(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    s.chars().all(|c| c.is_ascii_digit())
}

fn looks_like_id(s: &str) -> bool {
    // IDs are typically short numeric strings (1-12 digits)
    let len = s.len();
    len > 0 && len <= 12 && s.chars().all(|c| c.is_ascii_digit())
}

fn dedup_data_attr_fields(spec: &mut ApiSpec) {
    for (_name, entity) in spec.entities.iter_mut() {
        let fields = match &mut entity.fields {
            Some(f) => f,
            None => continue,
        };

        // Track which data-* attributes are already claimed
        let mut seen_data_attrs: HashMap<String, String> = HashMap::new(); // attr -> field_name
        let mut fields_to_remove: Vec<String> = Vec::new();

        for (fname, fdef) in fields.iter() {
            if let Some(attr) = &fdef.attribute {
                if attr.starts_with("data-") {
                    if let Some(existing_field) = seen_data_attrs.get(attr) {
                        // Duplicate: keep the one that looks more like an ID or the shorter name
                        let keep_current = if fname.ends_with("_id") && !existing_field.ends_with("_id") {
                            true
                        } else if existing_field.ends_with("_id") && !fname.ends_with("_id") {
                            false
                        } else {
                            fname.len() <= existing_field.len()
                        };
                        if keep_current {
                            fields_to_remove.push(existing_field.clone());
                            seen_data_attrs.insert(attr.clone(), fname.clone());
                        } else {
                            fields_to_remove.push(fname.clone());
                        }
                    } else {
                        seen_data_attrs.insert(attr.clone(), fname.clone());
                    }
                }
            }
        }

        for key in &fields_to_remove {
            fields.remove(key);
        }
    }
}

fn heuristic_entities(raw_data: &RawPageData) -> ApiSpec {
    let mut groups: HashMap<(String, String), Vec<&DataAttribute>> = HashMap::new();
    for attr in &raw_data.data_attributes {
        let class_str = attr.element_classes.join(".");
        let key = (attr.element_tag.clone(), class_str);
        groups.entry(key).or_default().push(attr);
    }

    let mut spec = ApiSpec {
        version: "1.0".to_string(),
        name: "heuristic".to_string(),
        base_url: None,
        info: None,
        types: BTreeMap::new(),
        enums: BTreeMap::new(),
        entities: BTreeMap::new(),
        pages: BTreeMap::new(),
        auth: None,
        rate_limits: None,
        drift_detection: None,
    };

    for ((tag, class), attrs) in &groups {
        if class.is_empty() {
            continue;
        }

        let entity_name = derive_entity_name(attrs);
        let fields = derive_fields(attrs);
        if fields.is_empty() {
            continue;
        }

        // Skip UI component noise
        let name_lower = entity_name.to_lowercase();
        let is_ui_noise = name_lower.starts_with("carousel")
            || name_lower.starts_with("modal")
            || name_lower.starts_with("navbar")
            || name_lower.starts_with("nav")
            || name_lower.starts_with("toggle")
            || name_lower.starts_with("dismiss")
            || name_lower.starts_with("fancybox")
            || name_lower.starts_with("cookie")
            || name_lower.starts_with("footer")
            || name_lower.starts_with("header")
            || name_lower.starts_with("section-type")
            || name_lower.starts_with("sort-")
            || name_lower.starts_with("items-per")
            || name_lower.starts_with("compact")
            || name_lower.starts_with("href")
            || name_lower.starts_with("sitekey")
            || name_lower.starts_with("target")
            || name_lower.starts_with("switcher")
            || name_lower.starts_with("collapse")
            || name_lower.starts_with("login")
            || name_lower.starts_with("counter")
            || name_lower.starts_with("promo")
            || name_lower.starts_with("app-data")
            || name_lower == "s"
            || name_lower == "fields";
        if is_ui_noise {
            continue;
        }

        let selector = if tag == "div" {
            format!(".{}", class)
        } else {
            format!("{}.{}", tag, class)
        };

        spec.entities.insert(
            entity_name,
            EntityDef {
                description: Some(format!("Auto-detected from {} elements", class)),
                list_selector: Some(selector),
                fields: Some(fields),
            },
        );
    }

    spec
}

fn derive_entity_name(attrs: &[&DataAttribute]) -> String {
    // Collect all data-* suffixes (strip "data-" prefix)
    let mut suffixes: Vec<String> = attrs
        .iter()
        .map(|a| {
            a.attribute_name
                .strip_prefix("data-")
                .unwrap_or(&a.attribute_name)
                .to_string()
        })
        .collect();
    suffixes.sort();
    suffixes.dedup();

    // Look for data-*-id patterns to derive entity name
    let id_patterns: Vec<&str> = suffixes
        .iter()
        .filter(|s| s.ends_with("-id") || s.ends_with("_id"))
        .map(|s| {
            s.strip_suffix("-id")
                .or_else(|| s.strip_suffix("_id"))
                .unwrap_or(s)
        })
        .collect();

    if let Some(name_part) = id_patterns.first() {
        // Map common patterns to entity names
        let mapped = match *name_part {
            "game" => "Game".to_string(),
            "user" => "User".to_string(),
            "lot" => "Lot".to_string(),
            "order" => "Order".to_string(),
            "chat" => "Chat".to_string(),
            "offer" => "Offer".to_string(),
            "server" => "Server".to_string(),
            "item" => "Item".to_string(),
            "category" => "Category".to_string(),
            "shop" => "Shop".to_string(),
            "review" => "Review".to_string(),
            "payment" => "Payment".to_string(),
            "trade" => "Trade".to_string(),
            "listing" => "Listing".to_string(),
            "product" => "Product".to_string(),
            "seller" => "Seller".to_string(),
            "buyer" => "Buyer".to_string(),
            other => {
                // PascalCase the name part
                let mut chars = other.chars();
                match chars.next() {
                    Some(first) => {
                        let upper: String = first.to_uppercase().collect();
                        format!("{}{}", upper, chars.as_str())
                    }
                    None => other.to_string(),
                }
            }
        };
        return mapped;
    }

    // Fallback: derive from the most common data-* suffix
    if let Some(most_common) = suffixes.first() {
        let first_char = most_common.chars().next();
        if let Some(c) = first_char {
            let upper: String = c.to_uppercase().collect();
            return format!("{}{}", &upper, &most_common[c.len_utf8()..]);
        }
    }

    "UnknownEntity".to_string()
}

fn derive_fields(attrs: &[&DataAttribute]) -> BTreeMap<String, FieldDef> {
    let mut fields = BTreeMap::new();

    // Group by attribute_name to get unique data-* attributes
    let mut by_name: HashMap<String, Vec<&DataAttribute>> = HashMap::new();
    for attr in attrs {
        by_name
            .entry(attr.attribute_name.clone())
            .or_default()
            .push(attr);
    }

    for (attr_name, attr_samples) in &by_name {
        let field_name = attr_name
            .strip_prefix("data-")
            .unwrap_or(attr_name)
            .replace('-', "_");

        // Skip very generic attributes
        if field_name == "id" || field_name.is_empty() {
            continue;
        }

        // Type inference from sample values
        let sample_value = attr_samples
            .first()
            .map(|a| a.value.as_str())
            .unwrap_or("");

        let field_type = if sample_value.starts_with("http://")
            || sample_value.starts_with("https://")
            || sample_value.starts_with("//")
        {
            "Url".to_string()
        } else if sample_value.ends_with(".jpg")
            || sample_value.ends_with(".png")
            || sample_value.ends_with(".webp")
            || sample_value.ends_with(".svg")
            || sample_value.ends_with(".gif")
        {
            "Url".to_string()
        } else if looks_numeric(sample_value) || looks_like_id(sample_value) {
            "u32".to_string()
        } else if sample_value == "true" || sample_value == "false" {
            "bool".to_string()
        } else {
            "String".to_string()
        };

        let transform = if field_name.ends_with("_id") && field_type == "u32" {
            Some("parse_id_from_url".to_string())
        } else if field_name.contains("price") && field_type == "String" {
            Some("parse_price".to_string())
        } else {
            None
        };

        fields.insert(
            field_name,
            FieldDef {
                r#type: field_type,
                nullable: Some(false),
                selector: None,
                attribute: Some(attr_name.clone()),
                transform,
                description: None,
            },
        );
    }

    fields
}

fn merge_heuristic(llm_spec: &mut ApiSpec, heuristic: ApiSpec) {
    let mut added = 0;
    for (name, entity) in heuristic.entities {
        // Check for conflict: same list_selector or similar name
        let conflict = llm_spec.entities.values().any(|e| {
            e.list_selector.is_some()
                && e.list_selector == entity.list_selector
        }) || llm_spec.entities.contains_key(&name);

        if !conflict {
            llm_spec.entities.insert(name, entity);
            added += 1;
        }
    }
    if added > 0 {
        eprintln!("  Heuristic: added {} entities from data-* patterns", added);
    }
}

fn extract_yaml(text: &str) -> String {
    let cleaned = text.trim();

    let yaml_str = if let Some(start) = cleaned.find("```yaml") {
        let after_fence = &cleaned[start + 7..];
        if let Some(end) = after_fence.find("```") {
            after_fence[..end].trim().to_string()
        } else {
            cleaned.to_string()
        }
    } else if let Some(start) = cleaned.find("```") {
        let after_fence = &cleaned[start + 3..];
        let content_start = after_fence.find('\n').map(|i| i + 1).unwrap_or(0);
        let content = &after_fence[content_start..];
        if let Some(end) = content.find("```") {
            content[..end].trim().to_string()
        } else {
            cleaned.to_string()
        }
    } else if let Some(start) = cleaned.find("version:") {
        cleaned[start..].trim().to_string()
    } else {
        cleaned.to_string()
    };

    deduplicate_yaml_keys(&yaml_str)
}

fn deduplicate_yaml_keys(yaml: &str) -> String {
    let top_keys = ["version:", "name:", "base_url:", "types:", "enums:", "entities:", "pages:", "auth:", "rate_limits:", "drift_detection:"];
    let mut seen_keys: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut result_lines: Vec<String> = Vec::new();
    let mut skip_until_next_key = false;

    for line in yaml.lines() {
        let trimmed = line.trim();
        let is_top_key = top_keys.iter().any(|k| trimmed.starts_with(k));

        if is_top_key {
            let key = trimmed.split(':').next().unwrap_or("").trim().to_string();
            if seen_keys.contains(&key) {
                skip_until_next_key = true;
                continue;
            }
            seen_keys.insert(key);
            skip_until_next_key = false;
        } else if skip_until_next_key {
            continue;
        }

        result_lines.push(line.to_string());
    }

    result_lines.join("\n")
}
