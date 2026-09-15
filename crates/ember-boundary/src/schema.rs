//! The template-free renderer used to validate JSON data in this phase.

use serde_json::{Map, Value, json};

use crate::{Boundary, Description, Field, Shape, TypeRef, VariantShape, View, Wide};

/// Render a boundary type as JSON Schema draft 2020-12.
#[must_use]
pub fn json_schema<T: Boundary>(view: View) -> Value {
    let mut schema = describe(&T::DESCRIPTION, view);
    let Value::Object(object) = &mut schema else {
        return schema;
    };
    object.insert(
        "$schema".into(),
        json!("https://json-schema.org/draft/2020-12/schema"),
    );
    object.insert("title".into(), json!(T::DESCRIPTION.name));
    schema
}

fn describe(description: &Description, view: View) -> Value {
    match description.shape {
        Shape::Object(fields) => object(fields, view, &[], description.deny_unknown_fields),
        Shape::Enum { tag, variants } => {
            let object =
                |fields, tag| object(fields, view, &[tag], description.deny_unknown_fields);
            let alternatives = variants
                .iter()
                .map(|variant| match (tag, variant.shape) {
                    (None, VariantShape::Unit) => json!({ "const": variant.name }),
                    (Some(tag), VariantShape::Unit) => object(&[], (tag, variant.name)),
                    (Some(tag), VariantShape::Object(fields)) => {
                        object(fields, (tag, variant.name))
                    }
                    (Some(tag), VariantShape::Intersection(ty)) => {
                        intersection(tag, variant.name, ty, view, description.deny_unknown_fields)
                    }
                    _ => unreachable!("derive excludes unsupported enum representations"),
                })
                .collect::<Vec<_>>();
            json!({ "oneOf": alternatives })
        }
    }
}

fn object(fields: &[Field], view: View, tags: &[(&str, &str)], deny_unknown_fields: bool) -> Value {
    let mut properties = Map::new();
    let mut required = Vec::new();
    for &(key, value) in tags {
        properties.insert(key.into(), json!({ "const": value }));
        required.push(Value::String(key.into()));
    }
    for field in fields {
        properties.insert(field.name.into(), type_schema(field.ty, view));
        let optional = match view {
            View::Input => field.input_optional,
            View::Output => field.output_optional,
        };
        if !optional {
            required.push(Value::String(field.name.into()));
        }
    }
    let additional_properties = matches!(view, View::Input) && !deny_unknown_fields;
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": additional_properties
    })
}

fn intersection(
    outer_key: &str,
    outer_value: &str,
    ty: TypeRef,
    view: View,
    deny_unknown_fields: bool,
) -> Value {
    let TypeRef::Named {
        describe: nested, ..
    } = ty
    else {
        unreachable!("derive permits only a named intersection payload");
    };
    let nested = nested();
    let deny_unknown_fields = deny_unknown_fields || nested.deny_unknown_fields;
    let outer = (outer_key, outer_value);
    let object2 = |fields, inner| object(fields, view, &[outer, inner], deny_unknown_fields);
    match nested.shape {
        Shape::Object(fields) => object(fields, view, &[outer], deny_unknown_fields),
        Shape::Enum {
            tag: Some(inner_key),
            variants,
        } => {
            let alternatives = variants
                .iter()
                .map(|variant| match variant.shape {
                    VariantShape::Unit => object2(&[], (inner_key, variant.name)),
                    VariantShape::Object(fields) => object2(fields, (inner_key, variant.name)),
                    VariantShape::Intersection(_) => {
                        unreachable!("nested intersections are outside the accepted subset")
                    }
                })
                .collect::<Vec<_>>();
            json!({ "oneOf": alternatives })
        }
        Shape::Enum { tag: None, .. } => {
            unreachable!("intersection payload must describe an object")
        }
    }
}

fn type_schema(ty: TypeRef, view: View) -> Value {
    match ty {
        TypeRef::Bool => json!({ "type": "boolean" }),
        TypeRef::String => json!({ "type": "string" }),
        TypeRef::Integer { signed, bits, wide } => integer(signed, bits, wide),
        TypeRef::Number { .. } => json!({ "type": "number" }),
        TypeRef::Nullable(inner) => {
            json!({ "anyOf": [type_schema(*inner, view), { "type": "null" }] })
        }
        TypeRef::Sequence(inner) => json!({ "type": "array", "items": type_schema(*inner, view) }),
        TypeRef::Array(inner, len) => json!({
            "type": "array", "items": type_schema(*inner, view), "minItems": len, "maxItems": len
        }),
        TypeRef::Tuple(items) => json!({
            "type": "array",
            "prefixItems": items.iter().map(|item| type_schema(*item, view)).collect::<Vec<_>>(),
            "minItems": items.len(),
            "maxItems": items.len()
        }),
        TypeRef::Named {
            describe: nested, ..
        } => describe(nested(), view),
    }
}

fn integer(signed: bool, bits: u8, wide: Option<Wide>) -> Value {
    let mut schema = if signed {
        let maximum = (1_i128 << (bits - 1)) - 1;
        json!({ "type": "integer", "minimum": -maximum - 1, "maximum": maximum })
    } else {
        let maximum = (1_u128 << bits) - 1;
        json!({ "type": "integer", "minimum": 0, "maximum": maximum })
    };
    let Value::Object(object) = &mut schema else {
        unreachable!("integer schemas are objects")
    };
    match wide {
        Some(Wide::Exact) => {
            object.insert("format".into(), json!("int64-exact"));
            object.insert(
                "description".into(),
                json!(
                    "Exact 64-bit integer identity; parse without a JavaScript number intermediate."
                ),
            );
        }
        Some(Wide::Precise { bound }) => {
            object.insert(
                "description".into(),
                json!(format!(
                    "64-bit integer represented as a JavaScript number; precision bound: {bound}."
                )),
            );
        }
        None => {}
    }
    schema
}
