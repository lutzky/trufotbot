use chrono::NaiveDateTime;
use serde::Serialize;
use sqlx::{FromRow, SqlitePool}; // Added SqlitePool

#[derive(FromRow, Serialize, Debug, PartialEq)]
pub struct Patient {
    pub id: i64,
    pub telegram_group_id: Option<i64>,
    pub name: String,
}

impl Patient {
    /// Fetches a patient by their ID from the database.
    pub async fn get(db: &SqlitePool, patient_id: i64) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            Patient,
            r"SELECT id, name, telegram_group_id FROM patients WHERE id = ?",
            patient_id
        )
        .fetch_optional(db)
        .await
    }
}

#[derive(FromRow, Serialize, Debug)]
pub struct Medication {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
}

impl Medication {
    pub async fn get(db: &SqlitePool, medication_id: i64) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            Medication,
            r"SELECT id, name, description FROM medications WHERE id = ?",
            medication_id
        )
        .fetch_optional(db)
        .await
    }
}

#[derive(FromRow, Serialize)]
pub struct Reminder {
    pub patient_id: i64,
    pub medication_id: i64,
    pub hour: Option<u8>,
}

#[derive(FromRow, Serialize)]
pub struct Dose {
    pub id: i64,
    pub patient_id: i64,
    pub medication_id: i64,
    pub quantity: f64,
    pub taken_at: NaiveDateTime,
    pub noted_by_user: Option<String>,
}
