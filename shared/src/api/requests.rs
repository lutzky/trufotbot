use serde::{Deserialize, Serialize};

/// Request for POST `/api/patients`, PUT `/api/patients/{patient_id}`
#[derive(Deserialize, Serialize, PartialEq, Debug)]
pub struct PatientCreateRequest {
    pub name: String,
    pub telegram_group_id: Option<i64>,
}

#[derive(Default, Deserialize, Serialize)]
pub struct CreateDoseQueryParams {
    pub reminder_message_id: Option<i32>,
}
