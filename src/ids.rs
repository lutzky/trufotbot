// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::str::FromStr;

use serde::{Deserialize, Serialize};
use teloxide::types::ChatId;
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

/// A Telegram chat (group) ID.
///
/// Telegram IDs are 64-bit integers (supergroups look like `-100xxxxxxxxxx`). In JSON they are
/// always a decimal *string* rather than a number, so JavaScript clients can't silently lose
/// precision above 2^53. Numbers are rejected on input. In the database it's a plain integer.
#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[serde(try_from = "String", into = "String")]
#[schema(value_type = String, pattern = r"^-?[0-9]+$", example = "-1001234567890")]
#[sqlx(transparent)]
pub struct TelegramGroupId(pub i64);

impl From<i64> for TelegramGroupId {
    fn from(value: i64) -> Self {
        TelegramGroupId(value)
    }
}

impl From<ChatId> for TelegramGroupId {
    fn from(chat_id: ChatId) -> Self {
        TelegramGroupId(chat_id.0)
    }
}

impl From<TelegramGroupId> for ChatId {
    fn from(group_id: TelegramGroupId) -> Self {
        ChatId(group_id.0)
    }
}

impl From<TelegramGroupId> for String {
    fn from(group_id: TelegramGroupId) -> Self {
        group_id.to_string()
    }
}

impl std::fmt::Display for TelegramGroupId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for TelegramGroupId {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse().map(TelegramGroupId)
    }
}

impl TryFrom<String> for TelegramGroupId {
    type Error = std::num::ParseIntError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn telegram_group_id_is_a_json_string() {
        // The second ID is beyond 2^53, so it can't survive being a JS number.
        for id in [-1_001_234_567_890, -1_001_234_567_890_123_456] {
            let json = serde_json::to_string(&TelegramGroupId(id)).unwrap();
            assert_eq!(json, format!("\"{id}\""));
            assert_eq!(
                serde_json::from_str::<TelegramGroupId>(&json).unwrap(),
                TelegramGroupId(id)
            );
        }
    }

    #[test]
    fn telegram_group_id_rejects_json_numbers() {
        for json in ["-123", "-123.0"] {
            assert!(serde_json::from_str::<TelegramGroupId>(json).is_err());
        }
    }

    #[test]
    fn telegram_group_id_rejects_non_numeric_strings() {
        for json in [r#""""#, r#""abc""#] {
            assert!(serde_json::from_str::<TelegramGroupId>(json).is_err());
        }
    }
}
