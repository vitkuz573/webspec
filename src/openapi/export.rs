use crate::openapi::info::info_to_openapi;
use crate::openapi::schema::{type_expr_to_openapi_schema, SchemaInnerExt};
use crate::openapi::OpenapiError;
use crate::spec::ApiSpec;
use oas3::spec::{
    Components, Info, MediaType, ObjectOrReference, ObjectSchema, Operation, PathItem, Response,
    Schema, SchemaType, SchemaTypeSet, SecurityScheme, Server,
};
use oas3::Map;
use std::collections::BTreeMap;

pub fn webspec_to_openapi(spec: &ApiSpec) -> Result<oas3::Spec, OpenapiError> {
    let mut paths: Map<String, PathItem> = Map::new();
    for (page_name, page) in spec.pages.iter() {
        let url = page
            .url_pattern
            .clone()
            .or_else(|| page.url.clone())
            .unwrap_or_else(|| format!("/{{{page_name}}}"));
        let method = page.method.clone().unwrap_or_else(|| "GET".to_string());
        let operation = build_operation(spec, page, page_name)?;

        let mut path_item = paths.get(&url).cloned().unwrap_or_default();
        match method.to_ascii_uppercase().as_str() {
            "GET" => path_item.get = Some(operation),
            "POST" => path_item.post = Some(operation),
            "PUT" => path_item.put = Some(operation),
            "DELETE" => path_item.delete = Some(operation),
            "PATCH" => path_item.patch = Some(operation),
            _ => path_item.get = Some(operation),
        }
        paths.insert(url, path_item);
    }

    let mut components = Components::default();
    components.schemas = build_schemas(spec)?;
    components.security_schemes = build_security_schemes(spec).unwrap_or_default();

    let mut oas = oas3::Spec {
        openapi: "3.1.0".to_string(),
        info: build_info(spec),
        servers: build_servers(spec),
        paths: Some(paths),
        components: Some(components),
        security: Vec::new(),
        tags: Vec::new(),
        webhooks: Map::new(),
        external_docs: None,
        extensions: Map::new(),
    };

    if let Some(info_val) = &spec.info {
        merge_info_extensions(info_val, &mut oas.info)?;
    }

    Ok(oas)
}

fn build_info(spec: &ApiSpec) -> Info {
    let api_info = spec
        .info
        .as_ref()
        .and_then(|v| serde_json::from_value::<crate::openapi::info::ApiInfo>(v.clone()).ok())
        .unwrap_or_default();
    info_to_openapi(&spec.name, &api_info)
}

fn merge_info_extensions(info_val: &serde_json::Value, info: &mut Info) -> Result<(), OpenapiError> {
    if let Some(obj) = info_val.as_object() {
        for (k, v) in obj {
            if k.starts_with("x-") {
                info.extensions.insert(k.clone(), v.clone());
            }
        }
    }
    Ok(())
}

fn build_servers(spec: &ApiSpec) -> Vec<Server> {
    if let Some(base_url) = &spec.base_url {
        vec![Server {
            url: base_url.clone(),
            description: None,
            variables: Map::new(),
            extensions: Map::new(),
        }]
    } else {
        Vec::new()
    }
}

fn build_operation(spec: &ApiSpec, page: &crate::spec::PageDef, page_name: &str) -> Result<Operation, OpenapiError> {
    let mut responses: Map<String, ObjectOrReference<Response>> = Map::new();
    let mut response = Response::default();
    response.description = Some("Successful response".to_string());

    if !page.entity.is_empty() {
        let schema = Schema::Object(Box::new(ObjectOrReference::Ref {
            ref_path: format!("#/components/schemas/{}", page.entity),
            summary: None,
            description: None,
        }));
        let mut media = MediaType::default();
        media.schema = Some(schema);
        response.content.insert("application/json".to_string(), media);
    }

    responses.insert("200".to_string(), ObjectOrReference::Object(response));

    let mut operation = Operation {
        operation_id: Some(page_name.to_string()),
        summary: page.description.clone(),
        description: page.description.clone(),
        responses: Some(responses),
        ..Default::default()
    };

    let mut extensions: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    if let Some(selector) = &page.list_selector {
        extensions.insert("x-webspec-list-selector".to_string(), serde_json::Value::String(selector.clone()));
    }

    if !page.entity.is_empty() {
        extensions.insert("x-webspec-entity".to_string(), serde_json::Value::String(page.entity.clone()));
    }
    for (k, v) in extensions {
        operation.extensions.insert(k, v);
    }

    Ok(operation)
}

fn build_schemas(spec: &ApiSpec) -> Result<Map<String, Schema>, OpenapiError> {
    let mut schemas: Map<String, Schema> = Map::new();

    for (name, entity) in spec.entities.iter() {
        let mut object = ObjectSchema::default();
        object.schema_type = Some(SchemaTypeSet::Single(SchemaType::Object));
        object.description = entity.description.clone();
        if let Some(fields) = &entity.fields {
            for (field_name, field) in fields.iter() {
                let mut schema = type_expr_to_openapi_schema(&field.r#type)?;
                if let Schema::Object(obj) = &mut schema {
                    if let ObjectOrReference::Object(inner) = obj.as_mut() {
                        inner.description = field.description.clone();
                    }
                }
                if field.nullable == Some(true) {
                    let inner = schema.inner_mut();
                    match inner.schema_type.clone() {
                        Some(SchemaTypeSet::Single(t)) => {
                            inner.schema_type = Some(SchemaTypeSet::Multiple(vec![SchemaType::Null, t]));
                        }
                        Some(SchemaTypeSet::Multiple(mut ts)) => {
                            if !ts.contains(&SchemaType::Null) {
                                ts.push(SchemaType::Null);
                            }
                            inner.schema_type = Some(SchemaTypeSet::Multiple(ts));
                        }
                        None => {}
                    }
                }
                object.properties.insert(field_name.clone(), schema);
            }
        }
        schemas.insert(name.clone(), Schema::Object(Box::new(ObjectOrReference::Object(object))));
    }

    for (name, enum_def) in spec.enums.iter() {
        let mut object = ObjectSchema::default();
        object.schema_type = Some(SchemaTypeSet::Single(SchemaType::String));
        object.description = enum_def.description.clone();
        object.enum_values = enum_def
            .values
            .values()
            .map(|v| serde_json::Value::String(v.clone()))
            .collect();
        schemas.insert(name.clone(), Schema::Object(Box::new(ObjectOrReference::Object(object))));
    }

    for (name, type_mapping) in spec.types.iter() {
        let is_newtype = type_mapping.newtype == Some(true);
        if is_newtype {
            let rust_type = type_mapping.rust.clone().unwrap_or_else(|| "string".to_string());
            let mut inner = type_expr_to_openapi_schema(&rust_type)?;
            let mut object = ObjectSchema::default();
            object.schema_type = Some(SchemaTypeSet::Single(SchemaType::Object));
            if let Schema::Object(obj) = &mut inner {
                if let ObjectOrReference::Object(schema) = obj.as_mut() {
                    schema.title = Some(name.clone());
                }
            }
            object.properties.insert(
                "value".to_string(),
                inner,
            );
            schemas.insert(name.clone(), Schema::Object(Box::new(ObjectOrReference::Object(object))));
        } else {
            let rust_type = type_mapping.rust.clone().unwrap_or_else(|| "string".to_string());
            let schema = type_expr_to_openapi_schema(&rust_type)?;
            schemas.insert(name.clone(), schema);
        }
    }

    Ok(schemas)
}

fn build_security_schemes(spec: &ApiSpec) -> Option<Map<String, ObjectOrReference<SecurityScheme>>> {
    let auth = spec.auth.as_ref()?;
    let mut schemes = Map::new();
    match auth.r#type.as_deref().unwrap_or("none") {
        "cookie" => {
            schemes.insert(
                "cookieAuth".to_string(),
                ObjectOrReference::Object(SecurityScheme::ApiKey {
                    location: "cookie".to_string(),
                    name: auth.cookie_name.clone().unwrap_or_else(|| "session".to_string()),
                    description: Some("Cookie authentication".to_string()),
                }),
            );
        }
        "header" => {
            schemes.insert(
                "headerAuth".to_string(),
                ObjectOrReference::Object(SecurityScheme::ApiKey {
                    location: "header".to_string(),
                    name: auth.header_name.clone().unwrap_or_else(|| "Authorization".to_string()),
                    description: Some("Header authentication".to_string()),
                }),
            );
        }
        "bearer" => {
            schemes.insert(
                "bearerAuth".to_string(),
                ObjectOrReference::Object(SecurityScheme::Http {
                    scheme: "bearer".to_string(),
                    bearer_format: Some("JWT".to_string()),
                    description: Some("Bearer authentication".to_string()),
                }),
            );
        }
        _ => {}
    }
    Some(schemes)
}
