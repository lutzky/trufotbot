// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    api::dose::AvailableDose,
    ids::{MedicationId, PatientId, TelegramGroupId},
};

use super::{dose, medication, patient::Reminders, requests::PatientMedicationCreateRequest};

/// Returned by [`medication::create`](crate::handlers::medication::create).
#[derive(Serialize, Deserialize, Clone, ToSchema)]
#[schema(description = "Response returned when a new medication is created.")]
pub struct MedicationCreateResponse {
    pub id: MedicationId,
}

/// Returned by [`patients::create`](crate::handlers::patients::create).
#[derive(Serialize, Deserialize, Clone, ToSchema)]
#[schema(description = "Response returned when a new patient is created.")]
pub struct PatientCreateResponse {
    pub id: PatientId,
}

/// Returned by [`patients::get`](crate::handlers::patients::get).
#[derive(Serialize, Deserialize, Clone, ToSchema)]
#[schema(
    description = "Patient details including Telegram group configuration and active medications."
)]
pub struct PatientGetResponse {
    pub name: String,

    pub telegram_group_id: Option<TelegramGroupId>,
    pub medications: Vec<medication::MedicationSummary>,
}

/// Returned by [`doses::list`](crate::handlers::doses::list).
#[derive(Deserialize, Serialize, Clone, PartialEq, Debug, ToSchema)]
#[schema(
    description = "Medication details, recent doses, reminders, and upcoming dose schedule for a patient."
)]
pub struct PatientGetDosesResponse {
    pub patient_name: String,
    pub medication: PatientMedicationCreateRequest,
    pub doses: Vec<dose::Dose>,
    pub reminders: Reminders,

    /// Times and quantities for next doses. Empty if no limits have been configured.
    pub next_doses: Vec<AvailableDose>,
}

/// Returned by [`doses::get`](crate::handlers::doses::get).
#[derive(Deserialize, Serialize, Clone, PartialEq, Debug, ToSchema)]
#[schema(description = "Single dose record with associated medication and inventory details.")]
pub struct GetDoseResponse {
    pub patient_name: String,
    pub medication_name: String,
    pub inventory: Option<f64>,
    pub dose: dose::Dose,
}

/// Returned by [`status::get`](crate::handlers::status::get).
#[derive(Serialize, Deserialize, Clone, ToSchema)]
#[schema(description = "Server status and current time information.")]
pub struct StatusResponse {
    pub timezone: String,
    pub server_time: String,
}
