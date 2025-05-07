use serde::{Deserialize, Serialize};

use super::{dose, medication};

/// Response for GET `/api/patients/{patient_id}`.
#[derive(Serialize, Deserialize, Clone)]
pub struct PatientGetResponse {
    pub patient_id: i64,
    pub patient_name: String,
    pub medications: Vec<medication::MedicationSummary>,
}

/// Response for GET `/api/patients/{patient_id}/dose/{medication_id}`.
#[derive(Deserialize, Serialize, PartialEq, Debug)]
pub struct PatientGetDosesResponse {
    pub patient_name: String,
    pub medication_name: String,
    pub doses: Vec<dose::Dose>,
}
