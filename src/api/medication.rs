// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{ops::Deref, str::FromStr};

use chrono::{DateTime, Utc};
use color_eyre::eyre::{Result, bail};
use serde::{Deserialize, Serialize};
use sqlx::{Decode, Encode, Sqlite, Type, sqlite::SqliteArgumentValue};
use utoipa::ToSchema;

use crate::{api::dose::AvailableDose, ids::MedicationId};

#[derive(Serialize, Deserialize, Clone, PartialEq, ToSchema)]
pub struct MedicationSummary {
    pub id: MedicationId,
    pub name: String,
    pub last_taken_at: Option<DateTime<Utc>>,
    pub next_doses: Vec<AvailableDose>,
    pub inventory: Option<f64>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, ToSchema)]
pub struct DoseLimit {
    #[schema(examples(12))]
    pub hours: u16,

    #[schema(examples(2.5))]
    pub amount: f64,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, ToSchema, Default)]
pub struct DoseLimits(pub Vec<DoseLimit>);

impl FromStr for DoseLimit {
    type Err = color_eyre::eyre::Report;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let Some((hours, amount)) = s.split_once(":") else {
            bail!("Invalid dose-limit spec {s:?}");
        };
        Ok(DoseLimit {
            hours: hours.parse()?,
            amount: amount.parse()?,
        })
    }
}

impl Deref for DoseLimits {
    type Target = [DoseLimit];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl IntoIterator for DoseLimits {
    type Item = DoseLimit;

    type IntoIter = std::vec::IntoIter<DoseLimit>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl FromStr for DoseLimits {
    type Err = color_eyre::eyre::Report;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let limits = s
            .split(",")
            .filter(|s| !s.is_empty())
            .map(DoseLimit::from_str)
            .collect::<Result<_>>()?;
        Ok(Self(limits))
    }
}

impl DoseLimits {
    fn serialize_db_string(&self) -> String {
        self.iter()
            .map(|DoseLimit { hours, amount }| format!("{hours}:{amount}"))
            .collect::<Vec<String>>()
            .join(",")
    }
}

impl Type<Sqlite> for DoseLimits {
    fn type_info() -> <Sqlite as sqlx::Database>::TypeInfo {
        <String as Type<Sqlite>>::type_info()
    }
}

impl<'q> Encode<'q, Sqlite> for DoseLimits {
    fn encode_by_ref(
        &self,
        buf: &mut <Sqlite as sqlx::Database>::ArgumentBuffer<'q>,
    ) -> std::result::Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        let serialized = self.serialize_db_string();
        buf.push(SqliteArgumentValue::Text(serialized.into()));
        Ok(sqlx::encode::IsNull::No)
    }
}

impl<'q> Decode<'q, Sqlite> for DoseLimits {
    fn decode(
        value: <Sqlite as sqlx::Database>::ValueRef<'q>,
    ) -> std::result::Result<Self, sqlx::error::BoxDynError> {
        let s = <&str as Decode<Sqlite>>::decode(value)?;
        Ok(s.parse::<DoseLimits>()?)
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use crate::api::medication::{DoseLimit, DoseLimits};

    #[rstest(
        input_str,
        expected_dose_limits,
        case("", &[]),
        case("1:10.5", &[DoseLimit { hours: 1, amount: 10.5 }]),
        case("1:10.5,2:20", &[DoseLimit { hours: 1, amount: 10.5 }, DoseLimit { hours: 2, amount: 20.0 }]),
        case("3:15.123,4:25.0", &[DoseLimit { hours: 3, amount: 15.123 }, DoseLimit { hours: 4, amount: 25.0 }])
    )]
    fn test_parse_dose_limits(input_str: &str, expected_dose_limits: &[DoseLimit]) {
        println!("Expecting {input_str:?} -> {expected_dose_limits:?}");
        let result = input_str.parse::<DoseLimits>().unwrap();
        assert_eq!(*result, *expected_dose_limits);
    }

    #[rstest(
        want_string,
        dose_limits,
        case("", &[]),
        case("1:10.5", &[DoseLimit { hours: 1, amount: 10.5 }]),
        case("1:10.5,2:20", &[DoseLimit { hours: 1, amount: 10.5 }, DoseLimit { hours: 2, amount: 20.0 }]),
        case("3:15.123,4:25", &[DoseLimit { hours: 3, amount: 15.123 }, DoseLimit { hours: 4, amount: 25.0 }])
    )]
    fn test_serialize_db_string(dose_limits: &[DoseLimit], want_string: &str) {
        println!("Expecting {dose_limits:?} -> {want_string:?}");
        let result = DoseLimits(dose_limits.into()).serialize_db_string();
        assert_eq!(result, want_string);
    }
}
