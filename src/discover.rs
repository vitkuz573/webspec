use crate::analyzer::{self, RawPageData};
use crate::llm::client::LlmClient;
use crate::llm::prompts;
use crate::llm::ChatMessage;
use crate::spec::ApiSpec;
use std::collections::HashSet;
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
