use chrono::{DateTime, TimeZone, Utc};
use gloo_console::{error, warn};
use yew::{Html, html};

use shared::time::{local_display, time_ago};

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

pub fn humanize_html(t: &DateTime<Utc>) -> Html {
    html! {
        <>
            { time_ago(t) }
            <small style="font-size: 0.7em; color: var(--pico-muted-color)">
                { " " }
                { local_display(t) }
            </small>
        </>
    }
}
