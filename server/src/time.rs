use std::env;

use chrono::{DateTime, Utc};
use chrono_humanize::{Accuracy, HumanTime};

pub fn local_display(t: &DateTime<Utc>) -> String {
    t.with_timezone(&chrono::Local)
        .format("%F (%a) %H:%M")
        .to_string()
}

const FAKE_TIME_ENV_VAR: &str = "TRUFOTBOT_FAKE_NOW";

/// Use midnight on January 2nd as "now", so that January 1st can count as
/// "yesterday".
const FAKE_TIME_EPOCH: &str = "2025-01-02T00:00:00Z";

/// Sets the clock to [`FAKE_TIME_EPOCH`] For use in testing.
///
/// # Safety
///
/// Setting environment variables is, as it turns out, a race condition.
#[cfg(test)]
pub unsafe fn use_fake_time() {
    unsafe {
        env::set_var(FAKE_TIME_ENV_VAR, "yes");
    }
}

/// Returns the current time for non-testing code, or [`FAKE_TIME_EPOCH`] in tests.
pub fn now() -> DateTime<Utc> {
    if env::var(FAKE_TIME_ENV_VAR).is_ok() {
        DateTime::parse_from_rfc3339(FAKE_TIME_EPOCH)
            .unwrap()
            .into()
    } else {
        Utc::now()
    }
}

pub fn time_relative(from: &DateTime<Utc>, to: &DateTime<Utc>) -> String {
    let present_tense =
        HumanTime::from(*from - *to).to_text_en(Accuracy::Rough, chrono_humanize::Tense::Present);
    if present_tense == "now" {
        present_tense
    } else if from < to {
        format!("{present_tense} later")
    } else {
        format!("{present_tense} earlier")
    }
}
