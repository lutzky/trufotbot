#![cfg(test)]
// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

//! Shared test utilities: markdown escaping, timestamp parsing, URL builders,
//! and keyboard factories.

use chrono::{DateTime, Utc};
use teloxide::utils::markdown;

use crate::messenger::callbacks;

/// Markdown-escape a string
/// [`teloxide::utils::markdown::escape`].
pub fn md(s: &str) -> String {
    markdown::escape(s).to_string()
}

/// Parse an RFC 3339 timestamp into UTC
pub fn dt(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s).unwrap().to_utc()
}

/// Build a URL to a reminder (medication) page with message parameters.
///
/// We pass message_time as an i64 rather than a constructed time; this is to make it easier to
/// validate the literal expectations in tests.
pub fn reminder_url(
    base: &url::Url,
    patient_id: i64,
    medication_id: i64,
    message_id: i32,
    message_time: i64,
) -> url::Url {
    let mut url = base.clone();
    url.path_segments_mut()
        .unwrap()
        .push("patients")
        .push(&patient_id.to_string())
        .push("medications")
        .push(&medication_id.to_string());
    url.query_pairs_mut()
        .append_pair("message_id", &message_id.to_string())
        .append_pair("message_time", &message_time.to_string());
    url
}

/// Build the recorded-dose keyboard ("Edit... ✏️" + "Repeat 🔁")
pub fn dose_keyboard(
    patient_id: i64,
    medication_id: i64,
    dose_id: i64,
    quantity: f64,
    base_url: &url::Url,
) -> Vec<(&'static str, callbacks::Action)> {
    let mut url = base_url.clone();
    url.path_segments_mut()
        .unwrap()
        .push("patients")
        .push(&patient_id.to_string())
        .push("medications")
        .push(&medication_id.to_string())
        .push("doses")
        .push(&dose_id.to_string());

    vec![
        ("Edit... ✏️", callbacks::Action::Link { url }),
        (
            "Repeat 🔁",
            callbacks::Action::TakeNew {
                patient_id,
                medication_id,
                quantity,
            },
        ),
    ]
}
