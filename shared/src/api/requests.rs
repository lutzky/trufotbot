use serde::{Deserialize, Serialize};

/// Request for PATCH `/api/patients/{patient_id}`.
#[derive(Deserialize, Serialize, PartialEq, Debug)]
pub struct PatientUpdateRequest {
    pub name: Option<String>,
    pub telegram_group_id: Option<i64>,
}
