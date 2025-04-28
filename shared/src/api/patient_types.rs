use serde::Serialize;
use chrono::NaiveDateTime;

#[derive(Serialize)]
pub struct PatientMedicationMenuItem {
    pub id: i64,
    pub name: String,
    pub last_taken_at: Option<NaiveDateTime>,
}

#[derive(Serialize)]
pub struct PatientMedicationMenu {
    pub patient_id: i64,
    pub patient_name: String,
    pub medications: Vec<PatientMedicationMenuItem>,
}
