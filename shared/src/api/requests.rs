use serde::{Deserialize, Serialize};

/// Request for POST `/api/patients`, PUT `/api/patients/{patient_id}`
#[derive(Deserialize, Serialize, PartialEq, Debug)]
pub struct PatientCreateRequest {
    pub name: String,
    pub telegram_group_id: Option<i64>,
}

/// Request for PUT `/api/patients/{patient_id}/medications/{medication_id}
#[derive(Deserialize, Serialize, PartialEq, Debug)]
pub struct PatientMedicationCreateRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Default, Deserialize, Serialize)]
pub struct CreateDoseQueryParams {
    pub reminder_message_id: Option<i32>,
}
