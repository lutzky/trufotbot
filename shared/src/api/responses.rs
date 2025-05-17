use serde::{Deserialize, Serialize};

use super::{dose, medication};

/// Response for GET `/api/patients/{patient_id}`.
#[derive(Serialize, Deserialize, Clone)]
pub struct PatientGetResponse {
    pub name: String,
    pub telegram_group_id: Option<i64>,
    pub medications: Vec<medication::MedicationSummary>,
}

/// Response for GET `/api/patients/{patient_id}/medications/{medication_id}`.
#[derive(Deserialize, Serialize, Clone, PartialEq, Debug)]
pub struct PatientGetDosesResponse {
    pub patient_name: String,
    pub medication_name: String,
    pub doses: Vec<dose::Dose>,
}

/// Response for GET `/api/patients/{patient_id}/medications/{medication_id}/doses/{dose_id}`.
#[derive(Deserialize, Serialize, Clone, PartialEq, Debug)]
pub struct GetDoseResponse {
    pub patient_name: String,
    pub medication_name: String,
    pub dose: dose::Dose,
}
