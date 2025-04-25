use sqlx::FromRow;
use serde::{Deserialize, Serialize};
use chrono::NaiveDateTime;

#[derive(FromRow, Serialize, Debug, PartialEq)]
pub struct Patient {
    pub id: i64,
    // TODO: This should be optional in the DB too...
    pub telegram_group_id: Option<i64>,
    pub name: String,
}

#[derive(FromRow, Serialize)]
pub struct Medication {
    pub id: i64,
    pub name: String,
    pub description: String,
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

// TODO: Remove below

#[derive(FromRow, Serialize, Deserialize)]
pub struct IntakeRecord {
    pub id: i64,
    pub user_id: i64,
    pub medicine_id: i64,
    pub quantity: f64,
    pub taken_at: NaiveDateTime,
    pub noted_by_user_id: Option<i64>,
    pub telegram_message_id: Option<i64>,
}

#[derive(FromRow, Serialize)]
pub struct UserMedicineDetails {
    pub user_id: i64,
    pub medicine_id: i64,
    pub medicine_name: String,
    pub last_taken_at: Option<NaiveDateTime>,
    pub daily_reminder_hour: Option<i64>,
    pub telegram_group_id: Option<String>,
    pub default_quantity: Option<f64>,
}

#[derive(Deserialize)]
pub struct CreateIntake {
    pub quantity: f64,
    pub taken_at: NaiveDateTime, // Or use a String and parse it
    pub noted_by_user_id: Option<i64>,
}
