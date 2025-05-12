use chrono::{DateTime, TimeDelta, Utc};
use chrono_humanize::HumanTime;

pub fn local_display(t: &DateTime<Utc>) -> String {
    t.with_timezone(&chrono::Local)
        .format("%F (%a) %H:%M")
        .to_string()
}

pub fn time_ago(t: &DateTime<Utc>) -> String {
    let delta = clamp_if_less(*t - Utc::now(), TimeDelta::minutes(1));
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
