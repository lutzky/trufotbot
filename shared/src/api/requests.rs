use serde::{Deserialize, Serialize};

use super::{medication::DoseLimit, patient::Reminders};

/// Request for POST `/api/patients`, PUT `/api/patients/{patient_id}`
#[derive(Deserialize, Serialize, PartialEq, Debug)]
pub struct PatientCreateRequest {
    pub name: String,
    pub telegram_group_id: Option<i64>,
}

/// Request for POST `/api/patients/{patient_id}/medications`
#[derive(Deserialize, Serialize, PartialEq, Debug, Clone)]
pub struct PatientMedicationCreateRequest {
    pub name: String,
    pub description: Option<String>,
    pub dose_limits: Vec<DoseLimit>,
    pub inventory: Option<f64>,
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
