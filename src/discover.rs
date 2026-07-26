use crate::analyzer::{self, AnalysisResult, CandidateEntity, CandidateField, FieldType};
use crate::llm::client::LlmClient;
use crate::llm::prompts;
use crate::llm::ChatMessage;
use crate::spec::{ApiSpec, EntityDef, EnumDef, FieldDef, PageDef, TypeMapping};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

pub struct DiscoverConfig {
    pub url: String,
    pub api_url: String,
    pub api_key: String,
    pub model: String,
    pub output: Option<std::path::PathBuf>,
}

pub struct DiscoverResult {
    pub analysis: AnalysisResult,
    pub spec: ApiSpec,
    pub yaml: String,
}

pub async fn discover(config: DiscoverConfig) -> anyhow::Result<DiscoverResult> {
    eprintln!("Phase 1/4: Static analysis...");
    let analysis = analyzer::analyze_url(&config.url).await?;

    eprintln!("  Found {} entities, {} url patterns",
        analysis.entities.len(), analysis.url_patterns.len());

    let client = LlmClient::new(&config.api_url, &config.api_key, &config.model);

    eprintln!("Phase 2/4: LLM field naming...");
    let entities_with_names = llm_name_fields(&client, &analysis).await.unwrap_or_else(|e| {
        eprintln!("  LLM field naming failed ({e}), using static names");
        analysis.entities.clone()
    });

    eprintln!("Phase 3/4: LLM entity grouping...");
    let grouped_entities = llm_group_entities(&client, &entities_with_names, &analysis).await.unwrap_or_else(|e| {
        eprintln!("  LLM entity grouping failed ({e}), using static grouping");
        entities_with_names
    });

    eprintln!("Phase 4/4: YAML assembly...");
    let spec = assemble_spec(&config.url, &analysis.title, &grouped_entities, &analysis);
    let yaml = serde_yaml::to_string(&spec)?;

    Ok(DiscoverResult {
        analysis,
        spec,
        yaml,
    })
}

async fn llm_name_fields(
    client: &LlmClient,
    analysis: &AnalysisResult,
) -> anyhow::Result<Vec<CandidateEntity>> {
    if analysis.entities.is_empty() {
        return Ok(analysis.entities.clone());
    }

    let snippets = build_html_snippets(analysis);
    if snippets.is_empty() {
        return Ok(analysis.entities.clone());
    }

    let system_prompt = prompts::build_field_naming_prompt(&analysis.title, &snippets);
    let user_msg = "Return the JSON array now.";

    let messages = vec![
        ChatMessage { role: "system".to_string(), content: system_prompt },
        ChatMessage { role: "user".to_string(), content: user_msg.to_string() },
    ];

    let response = client.chat(messages).await?;
    let named_fields = parse_field_naming_response(&response);

    if named_fields.is_empty() {
        return Ok(analysis.entities.clone());
    }

    let mut entities = analysis.entities.clone();
    apply_named_fields(&mut entities, &named_fields);
    Ok(entities)
}

async fn llm_group_entities(
    client: &LlmClient,
    entities: &[CandidateEntity],
    analysis: &AnalysisResult,
) -> anyhow::Result<Vec<CandidateEntity>> {
    if entities.is_empty() {
        return Ok(entities.to_vec());
    }

    let fields_json = build_fields_json(entities);
    let system_prompt = prompts::build_entity_grouping_prompt(&analysis.title, &fields_json);
    let user_msg = "Return the JSON array now.";

    let messages = vec![
        ChatMessage { role: "system".to_string(), content: system_prompt },
        ChatMessage { role: "user".to_string(), content: user_msg.to_string() },
    ];

    let response = client.chat(messages).await?;
    let grouping = parse_entity_grouping_response(&response);

    if grouping.is_empty() {
        return Ok(entities.to_vec());
    }

    Ok(merge_entity_grouping(entities, &grouping))
}

fn build_html_snippets(analysis: &AnalysisResult) -> String {
    let mut snippets = String::new();
    for entity in &analysis.entities {
        for field in &entity.fields {
            if !field.sample_values.is_empty() {
                let samples: Vec<&str> = field.sample_values.iter().take(3).map(|s| s.as_str()).collect();
                snippets.push_str(&format!(
                    "Selector: `{}`\n  Attribute: {}\n  Sample values: {}\n\n",
                    field.css_selector,
                    field.attribute.as_deref().unwrap_or("text"),
                    samples.join(" | ")
                ));
            }
        }
    }
    snippets
}

fn build_fields_json(entities: &[CandidateEntity]) -> String {
    let mut fields: Vec<JsonValue> = Vec::new();
    for entity in entities {
        for field in &entity.fields {
            fields.push(serde_json::json!({
                "name": field.name,
                "selector": field.css_selector,
                "samples": field.sample_values.iter().take(3).collect::<Vec<_>>(),
            }));
        }
    }
    serde_json::to_string_pretty(&fields).unwrap_or_default()
}

#[derive(Debug, Clone)]
struct NamedField {
    selector: String,
    name: String,
    #[allow(dead_code)]
    field_type: String,
    transform: String,
}

fn parse_field_naming_response(response: &str) -> Vec<NamedField> {
    let json_str = extract_json(response);
    let parsed: Result<JsonValue, _> = serde_json::from_str(&json_str);

    match parsed {
        Ok(val) => {
            // New format: flat array [{"selector": ..., "name": ..., "type": ...}]
            let fields_arr = if let Some(arr) = val.as_array() {
                Some(arr)
            } else {
                // Fallback: wrapped {"fields": [...]}
                val.get("fields").and_then(|v| v.as_array())
            };

            match fields_arr {
                Some(arr) => arr.iter().filter_map(|f| {
                    Some(NamedField {
                        selector: f.get("selector")?.as_str()?.to_string(),
                        name: f.get("name")?.as_str()?.to_string(),
                        field_type: f.get("type").and_then(|v| v.as_str()).unwrap_or("String").to_string(),
                        transform: f.get("transform").and_then(|v| v.as_str()).unwrap_or("none").to_string(),
                    })
                }).collect(),
                None => Vec::new(),
            }
        }
        Err(_) => Vec::new(),
    }
}

fn apply_named_fields(entities: &mut Vec<CandidateEntity>, named_fields: &[NamedField]) {
    for entity in entities {
        for field in &mut entity.fields {
            if let Some(named) = named_fields.iter().find(|nf| nf.selector == field.css_selector) {
                field.name = named.name.clone();
                field.description = format!("transform: {}", named.transform);
            }
        }
    }
}

#[derive(Debug, Clone)]
struct EntityGroup {
    name: String,
    #[allow(dead_code)]
    description: String,
    list_selector: Option<String>,
    field_names: Vec<String>,
}

fn parse_entity_grouping_response(response: &str) -> Vec<EntityGroup> {
    let json_str = extract_json(response);
    let parsed: Result<JsonValue, _> = serde_json::from_str(&json_str);

    match parsed {
        Ok(val) => {
            // New format: flat array [{"name": ..., "fields": [...]}]
            let entities_arr = if let Some(arr) = val.as_array() {
                Some(arr)
            } else {
                // Fallback: wrapped {"entities": [...]}
                val.get("entities").and_then(|v| v.as_array())
            };

            match entities_arr {
                Some(arr) => arr.iter().filter_map(|e| {
                    let field_names = e.get("fields")
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.iter().filter_map(|f| f.as_str().map(String::from)).collect())
                        .unwrap_or_default();
                    Some(EntityGroup {
                        name: e.get("name")?.as_str()?.to_string(),
                        description: e.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        list_selector: e.get("list_selector").and_then(|v| v.as_str()).map(String::from),
                        field_names,
                    })
                }).collect(),
                None => Vec::new(),
            }
        }
        Err(_) => Vec::new(),
    }
}

fn merge_entity_grouping(
    original_entities: &[CandidateEntity],
    groups: &[EntityGroup],
) -> Vec<CandidateEntity> {
    let mut all_fields: HashMap<String, CandidateField> = HashMap::new();
    for entity in original_entities {
        for field in &entity.fields {
            all_fields.insert(field.name.clone(), field.clone());
        }
    }

    let mut result = Vec::new();
    for group in groups {
        let fields: Vec<CandidateField> = group.field_names.iter()
            .filter_map(|name| all_fields.get(name).cloned())
            .collect();

        if fields.is_empty() {
            continue;
        }

        let list_selector = group.list_selector.clone()
            .or_else(|| {
                original_entities.iter()
                    .find(|e| e.fields.iter().any(|f| group.field_names.contains(&f.name)))
                    .map(|e| e.list_selector.clone())
            })
            .unwrap_or_default();

        let item_count = original_entities.iter()
            .filter(|e| e.fields.iter().any(|f| group.field_names.contains(&f.name)))
            .map(|e| e.item_count)
            .max()
            .unwrap_or(1);

        result.push(CandidateEntity {
            name: group.name.clone(),
            list_selector,
            fields,
            item_count,
            confidence: 0.8,
        });
    }

    if result.is_empty() {
        return original_entities.to_vec();
    }

    result
}

fn assemble_spec(
    url: &str,
    title: &str,
    entities: &[CandidateEntity],
    analysis: &AnalysisResult,
) -> ApiSpec {
    let name = derive_spec_name(title, url);

    let mut types: HashMap<String, TypeMapping> = HashMap::new();
    types.insert("String".to_string(), TypeMapping {
        rust: Some("String".to_string()),
        typescript: Some("string".to_string()),
        python: Some("str".to_string()),
        go: Some("string".to_string()),
        java: Some("String".to_string()),
        newtype: None,
    });
    types.insert("U32".to_string(), TypeMapping {
        rust: Some("u32".to_string()),
        typescript: Some("number".to_string()),
        python: Some("int".to_string()),
        go: Some("uint32".to_string()),
        java: Some("int".to_string()),
        newtype: None,
    });
    types.insert("Price".to_string(), TypeMapping {
        rust: Some("f64".to_string()),
        typescript: Some("number".to_string()),
        python: Some("float".to_string()),
        go: Some("float64".to_string()),
        java: Some("double".to_string()),
        newtype: Some(true),
    });

    let mut enums: HashMap<String, EnumDef> = HashMap::new();
    let mut entities_map: HashMap<String, EntityDef> = HashMap::new();
    let mut pages: HashMap<String, PageDef> = HashMap::new();

    for entity in entities {
        let mut fields_map: HashMap<String, FieldDef> = HashMap::new();

        for field in &entity.fields {
            let type_str = map_field_type(&field.field_type);
            let transform = infer_transform(field);

            fields_map.insert(field.name.clone(), FieldDef {
                r#type: type_str,
                nullable: None,
                selector: Some(field.css_selector.clone()),
                attribute: field.attribute.clone(),
                transform,
                description: if field.description.is_empty() { None } else { Some(field.description.clone()) },
            });
        }

        entities_map.insert(entity.name.clone(), EntityDef {
            description: None,
            fields: Some(fields_map),
        });

        let page_key = entity.name.to_lowercase().replace(' ', "_");
        let url_pattern = analysis.url_patterns.iter()
            .find(|p| p.pattern.to_lowercase().contains(&page_key))
            .map(|p| p.pattern.clone());

        pages.insert(page_key, PageDef {
            description: None,
            entity: entity.name.clone(),
            url: None,
            url_pattern,
            list_selector: Some(entity.list_selector.clone()),
            method: None,
        });
    }

    for (_, entity_def) in &entities_map {
        if let Some(fields) = &entity_def.fields {
            for (_, field_def) in fields {
                if field_def.r#type == "Enum" {
                    let type_name = format!("{}Enum", field_def.selector.as_deref().unwrap_or("Unknown"));
                    enums.entry(type_name.clone()).or_insert_with(|| EnumDef {
                        description: None,
                        values: HashMap::new(),
                    });
                }
            }
        }
    }

    let base_url = extract_base_url_string(url);

    ApiSpec {
        version: "1.0".to_string(),
        name,
        base_url: Some(base_url),
        types,
        enums,
        entities: entities_map,
        pages,
        auth: None,
        rate_limits: None,
        drift_detection: None,
    }
}

fn derive_spec_name(title: &str, url: &str) -> String {
    if !title.is_empty() {
        let clean: String = title.chars()
            .filter(|c| c.is_alphanumeric() || *c == ' ')
            .collect::<String>()
            .split_whitespace()
            .take(3)
            .collect::<Vec<&str>>()
            .join("_");
        if !clean.is_empty() {
            return clean;
        }
    }

    if let Ok(parsed) = url::Url::parse(url) {
        let host = parsed.host_str().unwrap_or("unknown");
        let name: String = host.chars()
            .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '-')
            .collect();
        name.split('.').next().unwrap_or("unknown").to_string()
    } else {
        "unknown".to_string()
    }
}

fn extract_base_url_string(url: &str) -> String {
    if let Ok(parsed) = url::Url::parse(url) {
        format!("{}://{}", parsed.scheme(), parsed.host_str().unwrap_or(""))
    } else {
        let parts: Vec<&str> = url.splitn(3, '/').collect();
        if parts.len() >= 3 {
            format!("{}://{}", parts[0].trim_end_matches(':'), parts[1])
        } else {
            url.to_string()
        }
    }
}

fn map_field_type(ft: &FieldType) -> String {
    match ft {
        FieldType::String => "String".to_string(),
        FieldType::U32 => "U32".to_string(),
        FieldType::F64 => "F64".to_string(),
        FieldType::Bool => "Bool".to_string(),
        FieldType::Url => "Url".to_string(),
        FieldType::Timestamp => "Timestamp".to_string(),
        FieldType::Price => "Price".to_string(),
        FieldType::Enum(_) => "Enum".to_string(),
        FieldType::Id => "Id".to_string(),
    }
}

fn infer_transform(field: &CandidateField) -> Option<String> {
    match field.field_type {
        FieldType::Price => Some("parse_price".to_string()),
        FieldType::Timestamp => Some("parse_date".to_string()),
        FieldType::Url if field.css_selector.contains("a") => Some("parse_id_from_url".to_string()),
        _ => None,
    }
}

fn extract_json(text: &str) -> String {
    let cleaned = text.trim();

    // Handle markdown code blocks: ```json ... ``` or ``` ... ```
    if let Some(start) = cleaned.find("```") {
        let after_fence = &cleaned[start + 3..];
        // Skip optional language tag (json, etc.) up to the first newline
        let content_start = after_fence.find('\n').map(|i| i + 1).unwrap_or(0);
        let content = &after_fence[content_start..];
        if let Some(end) = content.find("```") {
            return content[..end].trim().to_string();
        }
    }

    // Try to find a JSON array first (our prompts return arrays)
    if let Some(start) = cleaned.find('[') {
        if let Some(end) = cleaned.rfind(']') {
            if end > start {
                return cleaned[start..=end].to_string();
            }
        }
    }
    // Fall back to JSON object
    if let Some(start) = cleaned.find('{') {
        if let Some(end) = cleaned.rfind('}') {
            if end > start {
                return cleaned[start..=end].to_string();
            }
        }
    }
    text.to_string()
}
