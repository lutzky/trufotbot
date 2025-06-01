use std::env;

use chrono::{DateTime, TimeDelta, Utc};
use chrono_humanize::HumanTime;

pub fn local_display(t: &DateTime<Utc>) -> String {
    t.with_timezone(&chrono::Local)
        .format("%F (%a) %H:%M")
        .to_string()
}

const FAKE_TIME_ENV_VAR: &str = "TRUFOTBOT_FAKE_NOW";

/// # Safety
///
/// Setting environment variables is, as it turns out, a race condition. Only
/// use in tests.
pub unsafe fn use_fake_time() {
    unsafe {
        env::set_var(FAKE_TIME_ENV_VAR, "yes");
    }
}

pub fn now() -> DateTime<Utc> {
    if env::var(FAKE_TIME_ENV_VAR).is_ok() {
        DateTime::parse_from_rfc3339("2023-04-05T07:07:08Z")
            .unwrap()
            .into()
    } else {
        Utc::now()
    }
}

pub fn time_ago(t: &DateTime<Utc>) -> String {
    let delta = clamp_if_less(*t - now(), TimeDelta::minutes(1));
    HumanTime::from(delta).to_string()
}

/// If delta is shorter than min, return 0, to avoid overly-precise HumanTime.
/// This is important because we input time with minute resolution.
fn clamp_if_less(delta: TimeDelta, min: TimeDelta) -> TimeDelta {
    if delta.abs() > min {
        delta
    } else {
        TimeDelta::seconds(0)
    }
}
