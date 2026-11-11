// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::ids::{MessageId, TelegramGroupId};

use super::{medication::DoseLimits, patient::Reminders};

/// Sent as the request body of [`patients::create`](crate::handlers::patients::create)
/// and [`patients::update`](crate::handlers::patients::update).
#[derive(Deserialize, Serialize, PartialEq, Eq, Debug, ToSchema)]
#[schema(
    description = "Patient configuration fields (name and Telegram group notification target)."
)]
pub struct PatientCreateRequest {
    pub name: String,
    pub telegram_group_id: Option<TelegramGroupId>,
}

/// Sent as the request body of [`medication::create`](crate::handlers::medication::create),
/// and nested in [`PatientMedicationUpdateRequest`].
#[derive(Deserialize, Serialize, PartialEq, Debug, Clone, ToSchema)]
#[schema(
    description = "Fields of a medication itself (shared across all patients), as opposed to per-patient settings such as reminders."
)]
pub struct PatientMedicationCreateRequest {
    pub name: String,
    pub description: Option<String>,
    pub dose_limits: DoseLimits,
    pub inventory: Option<f64>,
}

/// Sent as the request body of [`medication::update`](crate::handlers::medication::update).
#[derive(Deserialize, Serialize, PartialEq, Debug, ToSchema)]
#[schema(
    description = "Medication fields paired with patient-specific configuration (such as reminders) for one patient and medication."
)]
pub struct PatientMedicationUpdateRequest {
    pub medication: PatientMedicationCreateRequest,
    pub reminders: Reminders,
}

#[derive(Default, Deserialize, Serialize)]
pub struct CreateDoseQueryParams {
    pub reminder_message_id: Option<MessageId>,
    pub reminder_sent_time: Option<DateTime<Utc>>,
}
