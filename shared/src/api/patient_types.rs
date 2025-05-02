use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct MedicationMenuItem {
    pub id: i64,
    pub name: String,
    pub last_taken_at: Option<NaiveDateTime>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct MedicationMenu {
    pub patient_id: i64,
    pub patient_name: String,
    pub medications: Vec<MedicationMenuItem>,
}

#[derive(Deserialize)]
pub struct UpdateRequest {
    pub name: Option<String>,
    pub telegram_group_id: Option<i64>,
}

// TODO: Organize
#[derive(Deserialize, Serialize)]
pub struct CreateDose {
    pub quantity: f64,
    pub taken_at: NaiveDateTime, // Or use a String and parse it
    pub noted_by_user: Option<String>,
}
