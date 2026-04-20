// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use chrono::{DateTime, Utc};
use color_eyre::eyre::Result;

use crate::storage::Storage;

#[derive(Debug)]
struct LastDoseInfo {
    patient: String,
    medication: String,
    taken_at: Option<DateTime<Utc>>,
    quantity: Option<f64>,
}

impl LastDoseInfo {
    fn score(&self, query: &Query) -> (i64, i64) {
        let timestamp = self.taken_at.map_or(0, |ts| ts.timestamp());

        let patient_similarity = query
            .patient
            .as_ref()
            .map_or(0.0, |patient| strsim::jaro_winkler(patient, &self.patient));

        let medication_similarity = query.medication.as_ref().map_or(0.0, |medication| {
            strsim::jaro_winkler(medication, &self.medication)
        });

        (
            (1000.0 * (patient_similarity + medication_similarity)) as i64,
            timestamp,
        )
    }
}

async fn get_last_doses(storage: Storage) -> Result<Vec<LastDoseInfo>> {
    let res = sqlx::query!(
        r#"
        WITH RankedDoses AS
          (SELECT d.patient_id,
                  d.medication_id,
                  d.taken_at,
                  d.quantity,
                  ROW_NUMBER() OVER(PARTITION BY d.patient_id, d.medication_id
                                    ORDER BY d.taken_at DESC) rn
           FROM doses d
           WHERE quantity > 0)
        SELECT p.name AS patient,
               m.name AS medication,
               rd.taken_at AS taken_at,
               rd.quantity AS quantity
        FROM patients p
        JOIN medications m
        LEFT JOIN RankedDoses rd ON p.id = rd.patient_id
        AND m.id = rd.medication_id
        AND rn=1;
        "#
    )
    .fetch_all(&storage.pool)
    .await?;

    Ok(res
        .iter()
        .map(|res| LastDoseInfo {
            patient: res.patient.clone(),
            medication: res.medication.clone(),
            taken_at: res.taken_at.map(|naive_time| naive_time.and_utc()),
            quantity: res.quantity,
        })
        .collect())
}

fn quote_if_needed(s: &str) -> String {
    let escaped = format!("{s:?}");

    let Some(shlex_result) = shlex::split(s) else {
        return escaped;
    };

    let [word] = shlex_result.as_slice() else {
        return escaped;
    };

    word.to_string()
}

#[derive(Default, Debug)]
struct Query {
    patient: Option<String>,
    medication: Option<String>,
    quantity: Option<f64>,
    noted_by_user: Option<String>,
    time: Option<String>,
}

impl Query {
    fn parse(s: &str) -> Self {
        let (s, time) = match s.split_once(" @") {
            Some((cmd, time)) => (cmd, Some(time.to_owned())),
            None => (s, None),
        };

        let Some(parts) = shlex::split(s) else {
            return Default::default();
        };

        match parts.as_slice() {
            [patient] => Query {
                patient: Some(patient.to_string()),
                time,
                ..Default::default()
            },
            [patient, medication] => Query {
                patient: Some(patient.to_string()),
                medication: Some(medication.to_string()),
                time,
                ..Default::default()
            },
            [patient, medication, quantity] => Query {
                patient: Some(patient.to_string()),
                medication: Some(medication.to_string()),
                quantity: quantity.parse().ok(),
                time,
                ..Default::default()
            },
            [patient, medication, quantity, noted_by_user]
            | [patient, medication, quantity, _, noted_by_user] => Query {
                patient: Some(patient.to_string()),
                medication: Some(medication.to_string()),
                quantity: quantity.parse().ok(),
                noted_by_user: Some(noted_by_user.to_string()),
                time,
            },
            _ => Default::default(),
        }
    }
}

pub async fn autocomplete(storage: Storage, query: &str) -> Result<Vec<String>> {
    let query = Query::parse(query);

    let mut last_doses = get_last_doses(storage).await?;

    last_doses.sort_by_key(|dose_info| dose_info.score(&query));

    let result = last_doses
        .iter()
        .rev()
        .map(|info| {
            format!(
                "/record {} {} {}{}{}",
                quote_if_needed(&info.patient),
                quote_if_needed(&info.medication),
                query.quantity.unwrap_or(info.quantity.unwrap_or(1.0)),
                query
                    .noted_by_user
                    .as_ref()
                    .map_or(String::new(), |user| format!(" by {user}")),
                query
                    .time
                    .as_ref()
                    .map_or(String::new(), |time| format!(" @{time}")),
            )
        })
        .take(10)
        .collect();

    Ok(result)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rstest::rstest;
    use sqlx::SqlitePool;

    use super::*;

    #[rstest(
        case("hello", "hello"),
        case("hello world", r#""hello world""#),
        case("it's", r#""it's""#),
        case(r#"it"s"#, r#""it\"s""#)
    )]
    fn test_quote_if_needed(#[case] input: &str, #[case] want: &str) {
        let got = quote_if_needed(input);
        assert_eq!(got, want)
    }

    async fn test_autocomplete(db: SqlitePool, query: &str, want: &[&str]) {
        let storage = Storage { pool: db };
        let res = autocomplete(storage.clone(), query).await.unwrap();
        assert_eq!(res, want);
    }

    #[sqlx::test(fixtures("fixtures/patients.sql", "fixtures/medications.sql"))]
    async fn test_autocomplete_empty(db: SqlitePool) {
        test_autocomplete(
            db,
            "",
            &[
                "/record Carol Paracetamol 1",
                "/record Carol Metformin 1",
                "/record Carol Ibuprofen 1",
                "/record Carol Aspirin 1",
                "/record Carol Amoxicillin 1",
                "/record Bob Paracetamol 1",
                "/record Bob Metformin 1",
                "/record Bob Ibuprofen 1",
                "/record Bob Aspirin 1",
                "/record Bob Amoxicillin 1",
            ],
        )
        .await;
    }

    #[sqlx::test(fixtures("fixtures/patients.sql", "fixtures/medications.sql"))]
    async fn test_autocomplete_specific(db: SqlitePool) {
        test_autocomplete(
            db,
            "alic moxic 3", // cSpell: disable-line
            &[
                "/record Alice Amoxicillin 3",
                "/record Carol Amoxicillin 3",
                "/record Alice Aspirin 3",
                "/record Alice Metformin 3",
                "/record Alice Paracetamol 3",
                "/record Carol Aspirin 3",
                "/record Carol Metformin 3",
                "/record Carol Paracetamol 3",
                "/record Bob Amoxicillin 3",
                "/record Alice Ibuprofen 3",
            ],
        )
        .await;
    }

    #[sqlx::test(fixtures("fixtures/patients.sql", "fixtures/medications.sql"))]
    async fn test_autocomplete_specific_by(db: SqlitePool) {
        test_autocomplete(
            db,
            "alic moxic 3 by Bob", // cSpell: disable-line
            &[
                "/record Alice Amoxicillin 3 by Bob",
                "/record Carol Amoxicillin 3 by Bob",
                "/record Alice Aspirin 3 by Bob",
                "/record Alice Metformin 3 by Bob",
                "/record Alice Paracetamol 3 by Bob",
                "/record Carol Aspirin 3 by Bob",
                "/record Carol Metformin 3 by Bob",
                "/record Carol Paracetamol 3 by Bob",
                "/record Bob Amoxicillin 3 by Bob",
                "/record Alice Ibuprofen 3 by Bob",
            ],
        )
        .await;
    }

    #[sqlx::test(fixtures("fixtures/patients.sql", "fixtures/medications.sql"))]
    async fn test_autocomplete_specific_without_by(db: SqlitePool) {
        test_autocomplete(
            db,
            "alic moxic 3 Bob", // cSpell: disable-line
            &[
                "/record Alice Amoxicillin 3 by Bob",
                "/record Carol Amoxicillin 3 by Bob",
                "/record Alice Aspirin 3 by Bob",
                "/record Alice Metformin 3 by Bob",
                "/record Alice Paracetamol 3 by Bob",
                "/record Carol Aspirin 3 by Bob",
                "/record Carol Metformin 3 by Bob",
                "/record Carol Paracetamol 3 by Bob",
                "/record Bob Amoxicillin 3 by Bob",
                "/record Alice Ibuprofen 3 by Bob",
            ],
        )
        .await;
    }

    #[sqlx::test(fixtures("fixtures/patients.sql", "fixtures/medications.sql"))]
    async fn test_autocomplete_with_time(db: SqlitePool) {
        test_autocomplete(
            db,
            "alic moxic 3 @10:00", // cSpell: disable-line
            &[
                "/record Alice Amoxicillin 3 @10:00",
                "/record Carol Amoxicillin 3 @10:00",
                "/record Alice Aspirin 3 @10:00",
                "/record Alice Metformin 3 @10:00",
                "/record Alice Paracetamol 3 @10:00",
                "/record Carol Aspirin 3 @10:00",
                "/record Carol Metformin 3 @10:00",
                "/record Carol Paracetamol 3 @10:00",
                "/record Bob Amoxicillin 3 @10:00",
                "/record Alice Ibuprofen 3 @10:00",
            ],
        )
        .await;
    }

    #[sqlx::test(fixtures("fixtures/patients.sql", "fixtures/medications.sql"))]
    async fn test_autocomplete_with_time_and_by(db: SqlitePool) {
        test_autocomplete(
            db,
            "alic moxic 3 by Bob @10:00", // cSpell: disable-line
            &[
                "/record Alice Amoxicillin 3 by Bob @10:00",
                "/record Carol Amoxicillin 3 by Bob @10:00",
                "/record Alice Aspirin 3 by Bob @10:00",
                "/record Alice Metformin 3 by Bob @10:00",
                "/record Alice Paracetamol 3 by Bob @10:00",
                "/record Carol Aspirin 3 by Bob @10:00",
                "/record Carol Metformin 3 by Bob @10:00",
                "/record Carol Paracetamol 3 by Bob @10:00",
                "/record Bob Amoxicillin 3 by Bob @10:00",
                "/record Alice Ibuprofen 3 by Bob @10:00",
            ],
        )
        .await;
    }
}
