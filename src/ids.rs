// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// Defines an ID newtype that is transparent to serde and sqlx.
//
// The OpenAPI schema is pinned to `i32` rather than following the underlying
// integer type. A field-level `#[schema(format = Int32)]` cannot be used here:
// once a field is a `$ref` to a named schema, utoipa drops any sibling
// keywords. Pinning the type on the newtype itself keeps generated clients
// using plain numbers instead of `bigint`.
macro_rules! define_id {
    ($name:ident, $int_type:ident) => {
        #[derive(
            PartialEq, Eq, Hash, Clone, Copy, Debug, Serialize, Deserialize, ToSchema, sqlx::Type,
        )]
        #[schema(value_type = i32)]
        #[sqlx(transparent)]
        pub struct $name(pub $int_type);

        impl From<$int_type> for $name {
            fn from(value: $int_type) -> Self {
                $name(value)
            }
        }

        impl From<$name> for $int_type {
            fn from(id: $name) -> Self {
                id.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

define_id!(DoseId, i64);
define_id!(MedicationId, i64);
define_id!(PatientId, i64);

define_id!(MessageId, i32);
