use serde::{Deserialize, Serialize};

use super::{dose, medication, patient::Reminders, requests::PatientMedicationCreateRequest};

/// Response for POST `/api/medications/`.`
#[derive(Serialize, Deserialize, Clone)]
pub struct MedicationCreateResponse {
    pub id: i64,
}

/// Response for POST `/api/patients/`.`
#[derive(Serialize, Deserialize, Clone)]
pub struct PatientCreateResponse {
    pub id: i64,
}

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
    pub medication: PatientMedicationCreateRequest,
    pub doses: Vec<dose::Dose>,
    pub reminders: Reminders,
}

/// Response for GET `/api/patients/{patient_id}/medications/{medication_id}/doses/{dose_id}`.
#[derive(Deserialize, Serialize, Clone, PartialEq, Debug)]
pub struct GetDoseResponse {
    pub patient_name: String,
    pub medication_name: String,
    pub dose: dose::Dose,
}
