use chrono::{DateTime, TimeZone, Utc};
use gloo_console::{error, warn};
use web_sys::HtmlInputElement;
use yew::prelude::*;

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

#[derive(PartialEq, Properties)]
pub struct LocalTimeProps {
    pub onchange: Callback<chrono::DateTime<chrono::Utc>>,
    pub utc_time: chrono::DateTime<chrono::Utc>,
}

#[function_component(LocalTime)]
pub fn local_time_component(LocalTimeProps { onchange, utc_time }: &LocalTimeProps) -> Html {
    let on_input = {
        let on_change = onchange.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            if let Some(t) = crate::time::try_parse_as_local(&input.value()) {
                on_change.emit(t.to_utc());
            };
        })
    };

    let formatted_time = format!(
        "{}",
        utc_time.with_timezone(&chrono::Local).format("%FT%H:%M")
    );

    html! { <input type="datetime-local" value={formatted_time} step=60 oninput={on_input} /> }
}
