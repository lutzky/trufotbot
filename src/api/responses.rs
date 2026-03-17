use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::api::dose::AvailableDose;

use super::{dose, medication, patient::Reminders, requests::PatientMedicationCreateRequest};

/// Response for POST `/api/medications/`.`
#[derive(Serialize, Deserialize, Clone, ToSchema)]
pub struct MedicationCreateResponse {
    #[schema(format = Int32)]
    pub id: i64,
}

/// Response for POST `/api/patients/`.`
#[derive(Serialize, Deserialize, Clone, ToSchema)]
pub struct PatientCreateResponse {
    #[schema(format = Int32)]
    pub id: i64,
}

/// Response for GET `/api/patients/{patient_id}`.
#[derive(Serialize, Deserialize, Clone, ToSchema)]
pub struct PatientGetResponse {
    pub name: String,

    #[schema(format = Int32)]
    pub telegram_group_id: Option<i64>,
    pub medications: Vec<medication::MedicationSummary>,
}

/// Response for GET `/api/patients/{patient_id}/medications/{medication_id}`.
#[derive(Deserialize, Serialize, Clone, PartialEq, Debug, ToSchema)]
pub struct PatientGetDosesResponse {
    pub patient_name: String,
    pub medication: PatientMedicationCreateRequest,
    pub doses: Vec<dose::Dose>,
    pub reminders: Reminders,
    pub next_doses: Vec<AvailableDose>,
}

/// Response for GET `/api/patients/{patient_id}/medications/{medication_id}/doses/{dose_id}`.
#[derive(Deserialize, Serialize, Clone, PartialEq, Debug, ToSchema)]
pub struct GetDoseResponse {
    pub patient_name: String,
    pub medication_name: String,
    pub inventory: Option<f64>,
    pub dose: dose::Dose,
}

/// Response for GET `/api/status`.
#[derive(Serialize, Deserialize, Clone, ToSchema)]
pub struct StatusResponse {
    pub timezone: String,
    pub server_time: String,
}
