// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum Action {
    // Rename to avoid long callback json; see `telegram_sender::maybe_warn_about_long_callback`
    #[serde(rename = "Take")]
    TakeFromReminder {
        patient_id: i64,
        medication_id: i64,
        quantity: f64,
    },
    TakeNew {
        patient_id: i64,
        medication_id: i64,
        quantity: f64,
    },
    Link {
        url: url::Url,
    },
}
