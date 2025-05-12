use chrono::{DateTime, TimeDelta, TimeZone, Utc};
use chrono_humanize::HumanTime;
use gloo_console::{error, warn};
use yew::{Html, html};

pub fn local_display(t: &DateTime<Utc>) -> String {
    t.with_timezone(&chrono::Local)
        .format("%F (%a) %H:%M")
        .to_string()
}

pub fn try_parse_as_local(s: &str) -> Option<chrono::DateTime<chrono::Local>> {
    let naive = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M").ok()?;
    let local = chrono::Local.from_local_datetime(&naive);
    match local {
        chrono::offset::LocalResult::Single(t) => Some(t),

        // TODO(https://github.com/chronotope/chrono/issues/1701) These never
        // happen, result is always Single. Also, the UI for Ambiguous can be
        // better.
        chrono::offset::LocalResult::Ambiguous(_early, late) => {
            warn!("Ambiguous time due to DST:", s, "- picked later option");
            Some(late)
        }
        chrono::offset::LocalResult::None => {
            error!("Nonexistent time due to DST:", s);
            None
        }
    }
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

pub fn humanize_html(t: &DateTime<Utc>) -> Html {
    let delta = clamp_if_less(*t - Utc::now(), TimeDelta::minutes(1));
    let time_since = HumanTime::from(delta).to_string();
    html! {
        <>
            { time_since }
            <small style="font-size: 0.7em; color: var(--pico-muted-color)">
                { " (" }
                { crate::time::local_display(t) }
                { ")" }
            </small>
        </>
    }
}
