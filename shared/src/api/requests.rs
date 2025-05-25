use serde::{Deserialize, Serialize};

use super::patient::Reminders;

/// Request for POST `/api/patients`, PUT `/api/patients/{patient_id}`
#[derive(Deserialize, Serialize, PartialEq, Debug)]
pub struct PatientCreateRequest {
    pub name: String,
    pub telegram_group_id: Option<i64>,
}

/// Request for POST `/api/patients/{patient_id}/medications`
#[derive(Deserialize, Serialize, PartialEq, Debug)]
pub struct PatientMedicationCreateRequest {
    pub name: String,
    pub description: Option<String>,
}

/// Request for PUT `/api/patients/{patient_id}/medications`
#[derive(Deserialize, Serialize, PartialEq, Debug)]
pub struct PatientMedicationUpdateRequest {
    pub medication: PatientMedicationCreateRequest,
    pub reminders: Reminders,
}

#[derive(Default, Deserialize, Serialize)]
pub struct CreateDoseQueryParams {
    pub reminder_message_id: Option<i32>,
}
