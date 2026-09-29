// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use chrono::{DateTime, TimeZone, Utc};
use chrono_humanize::{Accuracy, HumanTime};

pub trait DateTimeExt {
    /// Formats timestamp in the local system timezone
    fn local_display(&self) -> String;

    /// Formats interval relative to a reference timestamp (e.g. "an hour earlier")
    fn relative_to(&self, reference: &DateTime<Utc>) -> String;
}

#[cfg(any(test, doc))]
tokio::task_local! {
    /// Current time for tests, in RFC3339 format "YYYY-mm-ddTHH:MM:SSZ"
    pub static FAKE_TIME: &str;
}

/// Returns the current time for non-testing code, or [`FAKE_TIME`] in tests. You must
/// set [`FAKE_TIME`] like so:
///
/// ```
/// FAKE_TIME.scope("2025-01-02T00:00:00Z", async {
///   // Your async test code here
/// }).await;
///
/// FAKE_TIME.sync_scope("2025-01-02T00:00:00Z", || {
///   // Your non-async test code here
/// });
/// ```
pub fn now() -> DateTime<Utc> {
    #[cfg(test)]
    if let Ok(t) = FAKE_TIME.try_with(|t| *t) {
        DateTime::parse_from_rfc3339(t).unwrap().to_utc()
    } else {
        panic!("FAKE_TIME must be set in tests before now() can be called");
    }

    #[cfg(not(test))]
    Utc::now()
}

/// Returns the current timezone for non-testing code, or `UTC` in tests.
pub fn local_timezone() -> impl TimeZone {
    #[cfg(not(test))]
    return chrono::Local;

    #[cfg(test)]
    return chrono_tz::UTC;
}

impl DateTimeExt for DateTime<Utc> {
    fn local_display(&self) -> String {
        self.with_timezone(&chrono::Local)
            .format("%F (%a) %H:%M")
            .to_string()
    }

    fn relative_to(&self, reference: &DateTime<Utc>) -> String {
        let present_tense = HumanTime::from(*self - *reference)
            .to_text_en(Accuracy::Rough, chrono_humanize::Tense::Present);
        if present_tense == "now" {
            present_tense
        } else if self < reference {
            format!("{present_tense} later")
        } else {
            format!("{present_tense} earlier")
        }
    }
}
