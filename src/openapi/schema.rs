use crate::openapi::OpenapiError;
use oas3::spec::{ObjectOrReference, ObjectSchema, Schema, SchemaType, SchemaTypeSet};
use oas3::Spec;

pub fn openapi_schema_to_type_expr(schema: &Schema, spec: &Spec) -> Result<String, OpenapiError> {
    let resolved = schema.resolve(spec).map_err(|e| OpenapiError::Parse(e.to_string()))?;

    let inner: &ObjectSchema = match &resolved {
        Schema::Boolean(_) => return Ok("bool".to_string()),
        Schema::Object(obj) => match obj.as_ref() {
            ObjectOrReference::Object(s) => s,
            ObjectOrReference::Ref { ref_path, .. } => {
                return Ok(ref_path.rsplit('/').next().unwrap_or(ref_path).to_string())
            }
        },
    };

    let mut nullable = false;
    let mut types: Vec<SchemaType> = Vec::new();

    match &inner.schema_type {
        Some(SchemaTypeSet::Single(t)) => types.push(*t),
        Some(SchemaTypeSet::Multiple(ts)) => {
            for t in ts {
                if *t == SchemaType::Null {
                    nullable = true;
                } else {
                    types.push(*t);
                }
            }
        }
        None => {
            if !inner.properties.is_empty()
                || inner.additional_properties.is_some()
            {
                types.push(SchemaType::Object);
            } else if inner.items.is_some() {
                types.push(SchemaType::Array);
            } else {
                types.push(SchemaType::String);
            }
        }
    }

    let expr = if types.len() == 1 {
        match types[0] {
            SchemaType::String => "string".to_string(),
            SchemaType::Number => "f64".to_string(),
            SchemaType::Integer => "i64".to_string(),
            SchemaType::Boolean => "bool".to_string(),
            SchemaType::Array => {
                if let Some(item_schema) = inner.items.as_ref() {
                    let item_expr = openapi_schema_to_type_expr(item_schema, spec)?;
                    format!("Vec<{item_expr}>")
                } else {
                    "Vec<string>".to_string()
                }
            }
            SchemaType::Object => {
                if let Schema::Object(obj) = schema {
                    if let ObjectOrReference::Ref { ref_path, .. } = obj.as_ref() {
                        if let Some(name) = ref_path.rsplit('/').next() {
                            return Ok(name.to_string());
                        }
                    }
                }
                if let Some(title) = inner.title.as_deref() {
                    title.to_string()
                } else {
                    "string".to_string()
                }
            }
                SchemaType::Null => "string".to_string(),
        }
    } else {
        "string".to_string()
    };

    Ok(if nullable { format!("Option<{expr}>") } else { expr })
}

pub trait SchemaInnerExt {
    fn inner(&self) -> &ObjectSchema;
    fn inner_mut(&mut self) -> &mut ObjectSchema;
}

impl SchemaInnerExt for Schema {
    fn inner(&self) -> &ObjectSchema {
        match self {
            Schema::Boolean(_) => {
                static EMPTY: std::sync::OnceLock<ObjectSchema> = std::sync::OnceLock::new();
                EMPTY.get_or_init(ObjectSchema::default)
            }
            Schema::Object(obj) => match obj.as_ref() {
                ObjectOrReference::Object(s) => s,
                ObjectOrReference::Ref { .. } => {
                    static EMPTY: std::sync::OnceLock<ObjectSchema> = std::sync::OnceLock::new();
                    EMPTY.get_or_init(ObjectSchema::default)
                }
            },
        }
    }

    fn inner_mut(&mut self) -> &mut ObjectSchema {
        match self {
            Schema::Boolean(_) => {
                let mut o = ObjectSchema::default();
                o.schema_type = Some(SchemaTypeSet::Single(SchemaType::Boolean));
                *self = Schema::Object(Box::new(ObjectOrReference::Object(o)));
                match self {
                    Schema::Object(obj) => match obj.as_mut() {
                        ObjectOrReference::Object(s) => s,
                        _ => unreachable!(),
                    },
                    _ => unreachable!(),
                }
            }
            Schema::Object(obj) => {
                let needs_replace = matches!(obj.as_ref(), ObjectOrReference::Ref { .. });
                if needs_replace {
                    let ref_path = if let ObjectOrReference::Ref { ref_path, .. } = obj.as_ref() {
                        ref_path.clone()
                    } else {
                        String::new()
                    };
                    let mut o = ObjectSchema::default();
                    o.title = Some(ref_path);
                    **obj = ObjectOrReference::Object(o);
                }
                match obj.as_mut() {
                    ObjectOrReference::Object(s) => s,
                    _ => unreachable!(),
                }
            }
        }
    }
}

pub fn type_expr_to_openapi_schema(expr: &str) -> Result<Schema, OpenapiError> {
    let expr = expr.trim();

    if expr.starts_with("Option<") && expr.ends_with('>') {
        let inner = &expr[7..expr.len() - 1];
        let mut schema = type_expr_to_openapi_schema(inner)?;
        let inner_obj: &mut ObjectSchema = match &mut schema {
            Schema::Object(obj) => match obj.as_mut() {
                ObjectOrReference::Object(s) => s,
                ObjectOrReference::Ref { .. } => return Ok(schema),
            },
            Schema::Boolean(_) => {
                let mut o = ObjectSchema::default();
                o.schema_type = Some(SchemaTypeSet::Multiple(vec![SchemaType::Null, SchemaType::Boolean]));
                return Ok(Schema::Object(Box::new(ObjectOrReference::Object(o))));
            }
        };
        match inner_obj.schema_type.clone() {
            Some(SchemaTypeSet::Single(t)) => {
                inner_obj.schema_type = Some(SchemaTypeSet::Multiple(vec![SchemaType::Null, t]));
            }
            Some(SchemaTypeSet::Multiple(mut ts)) => {
                if !ts.contains(&SchemaType::Null) {
                    ts.push(SchemaType::Null);
                }
                inner_obj.schema_type = Some(SchemaTypeSet::Multiple(ts));
            }
            None => {
                inner_obj.schema_type = Some(SchemaTypeSet::Multiple(vec![SchemaType::Null, SchemaType::String]));
            }
        }
        return Ok(schema);
    }

    if expr.starts_with("Vec<") && expr.ends_with('>') {
        let inner = &expr[4..expr.len() - 1];
        let item_schema = type_expr_to_openapi_schema(inner)?;
        let mut object = ObjectSchema::default();
        object.schema_type = Some(SchemaTypeSet::Single(SchemaType::Array));
        object.items = Some(Box::new(item_schema));
        return Ok(Schema::Object(Box::new(ObjectOrReference::Object(object))));
    }

    let object = match expr {
        "string" | "String" | "url" | "date" | "datetime" | "decimal" => {
            let mut o = ObjectSchema::default();
            o.schema_type = Some(SchemaTypeSet::Single(SchemaType::String));
            if expr == "url" {
                o.format = Some("uri".to_string());
            }
            o
        }
        "bool" => {
            let mut o = ObjectSchema::default();
            o.schema_type = Some(SchemaTypeSet::Single(SchemaType::Boolean));
            o
        }
        "u32" | "u64" | "i64" => {
            let mut o = ObjectSchema::default();
            o.schema_type = Some(SchemaTypeSet::Single(SchemaType::Integer));
            if expr == "u32" {
                o.format = Some("int32".to_string());
            } else {
                o.format = Some("int64".to_string());
            }
            o
        }
        "f64" => {
            let mut o = ObjectSchema::default();
            o.schema_type = Some(SchemaTypeSet::Single(SchemaType::Number));
            o.format = Some("double".to_string());
            o
        }
        name => {
            let mut o = ObjectSchema::default();
            o.schema_type = Some(SchemaTypeSet::Single(SchemaType::Object));
            o.title = Some(name.to_string());
            o
        }
    };

    Ok(Schema::Object(Box::new(ObjectOrReference::Object(object))))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec_from_schema(schema: Schema) -> Spec {
        let mut spec = Spec::default();
        let mut components = oas3::spec::Components::default();
        let mut schemas = oas3::Map::new();
        schemas.insert("Pet".to_string(), ObjectOrReference::Object(schema));
        components.schemas = Some(schemas);
        spec.components = Some(components);
        spec
    }

    #[test]
    fn primitive_string() {
        let mut o = ObjectSchema::default();
        o.schema_type = Some(SchemaTypeSet::Single(SchemaType::String));
        let s = Schema::Object(Box::new(ObjectOrReference::Object(o)));
        let spec = spec_from_schema(s.clone());
        assert_eq!(openapi_schema_to_type_expr(&s, &spec).unwrap(), "string");
    }

    #[test]
    fn primitive_bool() {
        let mut o = ObjectSchema::default();
        o.schema_type = Some(SchemaTypeSet::Single(SchemaType::Boolean));
        let s = Schema::Object(Box::new(ObjectOrReference::Object(o)));
        let spec = spec_from_schema(s.clone());
        assert_eq!(openapi_schema_to_type_expr(&s, &spec).unwrap(), "bool");
    }

    #[test]
    fn primitive_integer() {
        let mut o = ObjectSchema::default();
        o.schema_type = Some(SchemaTypeSet::Single(SchemaType::Integer));
        let s = Schema::Object(Box::new(ObjectOrReference::Object(o)));
        let spec = spec_from_schema(s.clone());
        assert_eq!(openapi_schema_to_type_expr(&s, &spec).unwrap(), "i64");
    }

    #[test]
    fn primitive_number() {
        let mut o = ObjectSchema::default();
        o.schema_type = Some(SchemaTypeSet::Single(SchemaType::Number));
        let s = Schema::Object(Box::new(ObjectOrReference::Object(o)));
        let spec = spec_from_schema(s.clone());
        assert_eq!(openapi_schema_to_type_expr(&s, &spec).unwrap(), "f64");
    }

    #[test]
    fn nullable_string() {
        let mut o = ObjectSchema::default();
        o.schema_type = Some(SchemaTypeSet::Multiple(vec![SchemaType::Null, SchemaType::String]));
        let s = Schema::Object(Box::new(ObjectOrReference::Object(o)));
        let spec = spec_from_schema(s.clone());
        assert_eq!(openapi_schema_to_type_expr(&s, &spec).unwrap(), "Option<string>");
    }

    #[test]
    fn array_of_strings() {
        let mut item = ObjectSchema::default();
        item.schema_type = Some(SchemaTypeSet::Single(SchemaType::String));
        let mut o = ObjectSchema::default();
        o.schema_type = Some(SchemaTypeSet::Single(SchemaType::Array));
        o.items = Some(Box::new(Schema::Object(Box::new(ObjectOrReference::Object(item)))));
        let s = Schema::Object(Box::new(ObjectOrReference::Object(o)));
        let spec = spec_from_schema(s.clone());
        assert_eq!(openapi_schema_to_type_expr(&s, &spec).unwrap(), "Vec<string>");
    }

    #[test]
    fn user_defined_ref() {
        let s: Schema = Schema::Object(Box::new(ObjectOrReference::Ref {
            reference: "#/components/schemas/Pet".to_string(),
            summary: None,
            description: None,
        }));
        let mut spec = Spec::default();
    let mut components = oas3::spec::Components::default();
        let mut schemas = oas3::Map::new();
        let mut o = ObjectSchema::default();
        o.schema_type = Some(SchemaTypeSet::Single(SchemaType::Object));
        schemas.insert("Pet".to_string(), ObjectOrReference::Object(Schema::Object(Box::new(ObjectOrReference::Object(o)))));
        components.schemas = Some(schemas);
        spec.components = Some(components);
        assert_eq!(openapi_schema_to_type_expr(&s, &spec).unwrap(), "Pet");
    }

    #[test]
    fn type_expr_to_openapi_primitives() {
        let s = type_expr_to_openapi_schema("string").unwrap();
        assert_eq!(s.inner().schema_type, Some(SchemaTypeSet::Single(SchemaType::String)));

        let s = type_expr_to_openapi_schema("bool").unwrap();
        assert_eq!(s.inner().schema_type, Some(SchemaTypeSet::Single(SchemaType::Boolean)));

        let s = type_expr_to_openapi_schema("i64").unwrap();
        assert_eq!(s.inner().schema_type, Some(SchemaTypeSet::Single(SchemaType::Integer)));

        let s = type_expr_to_openapi_schema("f64").unwrap();
        assert_eq!(s.inner().schema_type, Some(SchemaTypeSet::Single(SchemaType::Number)));
    }

    #[test]
    fn type_expr_to_openapi_option() {
        let s = type_expr_to_openapi_schema("Option<string>").unwrap();
        assert!(matches!(s.inner().schema_type, Some(SchemaTypeSet::Multiple(_))));
    }

    #[test]
    fn type_expr_to_openapi_vec() {
        let s = type_expr_to_openapi_schema("Vec<string>").unwrap();
        assert_eq!(s.inner().schema_type, Some(SchemaTypeSet::Single(SchemaType::Array)));
    }
}
