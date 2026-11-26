//! The shapes of the engine's data as JSON Schemas (feature `schema`: the editor's forms, pickers
//! and field help are made from them). A schema describes what a value looks like *as written*
//! (`serde_json::to_value`): its fields in order, what each holds, the variants of its enums, the
//! values serde fills in for fields a file leaves out, and each field's doc comment.
//!
//! Every type that is written to a file derives `schemars::JsonSchema` under the feature; the two
//! with a format of their own are described here.

use std::borrow::Cow;

use schemars::generate::SchemaSettings;
use schemars::{json_schema, JsonSchema, Schema, SchemaGenerator};

use crate::refactor::RefKind;

impl JsonSchema for crate::skin::SkinColor {
    fn schema_name() -> Cow<'static, str> {
        "SkinColor".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({ "type": "string", "format": "color", "description": "A colour: \"#rrggbb\" or \"#rrggbbaa\"." })
    }
}

impl JsonSchema for crate::fog::Explored {
    fn schema_name() -> Cow<'static, str> {
        "Explored".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({ "type": "string", "description": "The explored tiles of the maps, packed." })
    }
}

/// The schema of `T` as it is written (not the older forms it also reads).
pub fn of<T: JsonSchema>() -> Schema {
    SchemaSettings::draft2020_12().for_serialize().into_generator().into_root_schema_for::<T>()
}

/// The schema of a definition of `kind`; `None` for kinds without definitions (maps, skills).
pub fn of_kind(kind: RefKind) -> Option<Schema> {
    crate::with_def_kind!(kind, |T| Some(of::<T>()), else => None)
}

/// The project's settings (`project.ron`).
pub fn project() -> Schema {
    of::<crate::project::ProjectConfig>()
}

/// A placed object of a map.
pub fn entity() -> Schema {
    of::<crate::entity::EntityInstance>()
}

/// A zone of a map.
pub fn zone() -> Schema {
    of::<crate::map::Zone>()
}

/// A trigger (of a definition, a placed object, a zone or a map).
pub fn trigger() -> Schema {
    of::<crate::events::Trigger>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_definition_kind_has_a_schema_with_its_fields() {
        for kind in RefKind::ALL {
            let Some(s) = of_kind(kind) else { continue };
            let v = s.to_value();
            assert!(v["properties"]["id"].is_object() || v["properties"]["name"].is_object(), "{kind:?}: {v}");
        }
        let item = of_kind(RefKind::Item).unwrap().to_value();
        assert!(item["properties"]["name"]["description"].is_string() || item["properties"]["max_stack"]["description"].is_string(), "doc comments are the descriptions");
        assert!(project().to_value()["properties"].as_object().is_some_and(|p| p.len() > 10));
    }
}
