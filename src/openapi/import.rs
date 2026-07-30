use crate::openapi::info::info_from_openapi;
use crate::openapi::lossy::LossReport;
use crate::openapi::naming;
use crate::openapi::schema::openapi_schema_to_type_expr;
use crate::openapi::OpenapiError;
use crate::spec::{ApiSpec, EntityDef, FieldDef, PageDef};
use oas3::spec::{FromRef, ObjectOrReference};
use oas3::Spec;
use std::collections::BTreeMap;
use std::collections::BTreeSet;

pub fn openapi_to_webspec(oas: &Spec, report: &mut LossReport) -> Result<ApiSpec, OpenapiError> {
    let mut pages = BTreeMap::new();
    let mut entities = BTreeMap::new();
    let mut used_names = BTreeSet::new();

    if let Some(paths) = &oas.paths {
        for (path, item) in paths.iter() {
            for (method, op) in item.methods() {
                let page_name = naming::operation_page_name(path, method.as_str(), op.operation_id.as_deref(), &mut used_names);
                let entity_name = response_entity_name(oas, op, report)?;
                if let Some(name) = &entity_name {
                    entities.insert(name.clone(), build_entity_placeholder(name.clone()));
                }
                pages.insert(
                    page_name.clone(),
                    PageDef {
                        description: op.description.clone().or_else(|| op.summary.clone()),
                        entity: entity_name.unwrap_or_else(|| "String".to_string()),
                        url: None,
                        url_pattern: Some(path.clone()),
                        list_selector: None,
                        method: Some(method.to_string()),
                    },
                );
            }
        }
    }

    let component_schema_names: std::collections::HashSet<String> = oas
        .components
        .as_ref()
        .map(|c| c.schemas.keys().cloned().collect())
        .unwrap_or_default();

    if let Some(components) = &oas.components {
        if !components.schemas.is_empty() {
            for (name, schema) in components.schemas.iter() {
                let entity = schema_to_entity(oas, name, schema, report)?;
                entities.insert(name.to_string(), entity);
            }
        }
    }

    // Remove placeholder entities that were created from inline response schemas
    // unless their name matches a real component schema.
    let placeholder_names: Vec<String> = entities
        .iter()
        .filter(|(_, e)| e.description.as_deref() == Some("Inline response entity"))
        .map(|(k, _)| k.clone())
        .collect();
    for name in placeholder_names {
        if !component_schema_names.contains(&name) {
            entities.remove(&name);
        }
    }

    let base_url = oas
        .servers
        .first()
        .map(|s| s.url.clone())
        .unwrap_or_else(|| "/".to_string());

    if oas.servers.len() > 1 {
        report.unsupported("server variables", "only first server URL is kept; variables ignored");
    }

    let info_value = serde_json::to_value(info_from_openapi(&oas.info)).map_err(OpenapiError::from)?;
    let info_value = strip_null_values(info_value);

    Ok(ApiSpec {
        version: "1.0.0".to_string(),
        protocol: "webspec".to_string(),
        name: naming::sanitize_pascal_case(&oas.info.title),
        base_url: Some(base_url),
        info: if info_value.is_null() { None } else { Some(info_value) },
        types: BTreeMap::new(),
        enums: BTreeMap::new(),
        entities,
        pages,
        auth: None,
        rate_limits: None,
        drift_detection: None,
    })
}

fn response_entity_name(oas: &Spec, op: &oas3::spec::Operation, report: &mut LossReport) -> Result<Option<String>, OpenapiError> {
    let Some(responses) = &op.responses else {
        return Ok(None);
    };

    for (code, response_ref) in responses.iter() {
        if code != "200" {
            continue;
        }
        let response = match response_ref {
            ObjectOrReference::Object(r) => r.clone(),
            ObjectOrReference::Ref { ref_path, .. } => {
                oas3::spec::Response::from_ref(oas, ref_path).map_err(|e| OpenapiError::Parse(e.to_string()))?
            }
        };
        let content = response.content;
        if let Some(media) = content.get("application/json") {
            if let Some(schema) = &media.schema {
                let expr = openapi_schema_to_type_expr(schema, oas)?;
                return Ok(Some(expr));
            }
        }
        if !content.is_empty() {
            report.unsupported("non-JSON response", code);
        }
    }

    Ok(None)
}

fn build_entity_placeholder(_name: String) -> EntityDef {
    EntityDef {
        description: Some("Inline response entity".to_string()),
        list_selector: None,
        fields: Some(BTreeMap::new()),
    }
}

fn schema_to_entity(
    _oas: &Spec,
    name: &str,
    schema: &oas3::spec::Schema,
    report: &mut LossReport,
) -> Result<EntityDef, OpenapiError> {
    let schema = schema.resolve(_oas).map_err(|e| OpenapiError::Parse(e.to_string()))?;
    let inner = match &schema {
        oas3::spec::Schema::Object(obj) => match obj.as_ref() {
            ObjectOrReference::Object(s) => s,
            ObjectOrReference::Ref { ref_path, .. } => {
                return Ok(EntityDef {
                    description: Some(format!("Reference to {ref_path}")),
                    list_selector: None,
                    fields: None,
                });
            }
        },
        oas3::spec::Schema::Boolean(_) => {
            return Ok(EntityDef {
                description: Some(format!("Boolean schema {name}")),
                list_selector: None,
                fields: None,
            });
        }
    };

    let mut fields = BTreeMap::new();
    for (prop_name, prop_schema) in inner.properties.iter() {
        let expr = openapi_schema_to_type_expr(prop_schema, _oas).unwrap_or_else(|_| "string".to_string());
        fields.insert(
            prop_name.clone(),
            FieldDef {
                r#type: expr,
                nullable: Some(false),
                selector: Some(".openapi-body".to_string()),
                attribute: None,
                transform: None,
                description: None,
            },
        );
    }

    if !inner.all_of.is_empty() || !inner.any_of.is_empty() || !inner.one_of.is_empty() {
        report.unsupported("schema composition", name);
    }

    Ok(EntityDef {
        description: inner.description.clone(),
        list_selector: None,
        fields: Some(fields),
    })
}

fn strip_null_values(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(mut map) => {
            map.retain(|_, v| !v.is_null());
            serde_json::Value::Object(
                map.into_iter()
                    .map(|(k, v)| (k, strip_null_values(v)))
                    .collect(),
            )
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(strip_null_values).collect())
        }
        other => other,
    }
}
