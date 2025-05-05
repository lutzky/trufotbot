use anyhow::{Result, bail};
use chrono::{DateTime, Local, TimeZone};
use gloo_console::{error, info, warn};
use gloo_net::http::Request;
use web_sys::HtmlInputElement;
use yew::{Callback, Html, Properties, TargetCast, function_component, html, use_state};

#[derive(Properties, PartialEq)]
pub struct PatientMedicationDetailProps {
    pub patient_id: i64,
    pub medication: shared::api::patient_types::MedicationMenuItem,
    pub on_log_dose: Callback<()>,
}

fn try_parse_time_as_local(s: &str) -> Option<chrono::DateTime<chrono::Local>> {
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

async fn log_dose(
    patient_id: i64,
    medication_id: i64,
    utc_time: chrono::DateTime<chrono::Utc>,
) -> Result<()> {
    let api_url = format!("/api/patients/{}/doses/{}", patient_id, medication_id);
    info!(format!("Logging dose with utc_time {utc_time:?}"));
    let payload = shared::api::patient_types::CreateDose {
        quantity: 1.0, // TODO - Make this configurable
        taken_at: utc_time,
        noted_by_user: None, // TODO - Make this configurable
    };

    let response = Request::put(&api_url).json(&payload)?.send().await?;

    if response.ok() {
        info!("Dose logged successfully via API.");
        Ok(())
    } else {
        bail!("Failed to log dose: Status {}", response.status());
    }
}

#[function_component(PatientMedicationDetail)]
pub fn patient_medication_detail(
    PatientMedicationDetailProps {
        patient_id,
        medication,
        on_log_dose,
    }: &PatientMedicationDetailProps,
) -> Html {
    let last_taken = medication
        .last_taken_at
        .map(|dt| DateTime::<Local>::from(dt).format("%c %z").to_string())
        .unwrap_or_else(|| "Never taken".to_string());

    let time_taken = use_state(|| chrono::Local::now());
    let time_taken_fmt = format!("{}", time_taken.format("%FT%T"));

    let on_time_change = {
        let time_taken = time_taken.clone();
        Callback::from(move |e: yew::Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            if let Some(t) = try_parse_time_as_local(&input.value()) {
                time_taken.set(t);
            };
        })
    };

    let on_button_click = {
        let patient_id = *patient_id;
        let medication_id = medication.id;
        let on_log_dose = on_log_dose.clone();
        Callback::from(move |_| {
            let on_log_dose = on_log_dose.clone();
            let time_taken = time_taken.clone();
            wasm_bindgen_futures::spawn_local(async move {
                log_dose(patient_id, medication_id, time_taken.to_utc())
                    .await
                    .unwrap_or_else(|e| {
                        error!(format!("Failed to log dose: {}", e));
                    });
                on_log_dose.emit(());
            })
        })
    };

    html! {
        <div class="medication-item" key={medication.id}>
            <h3>{ &medication.name }</h3>
            <p>{"Last taken: "}{ last_taken }</p>
            // TODO: Add chrono-humanize to show "how long ago" this is
            <div style="display: flex; gap: 8px; align-items: center;">
                <input
                    type="datetime-local"
                    value={time_taken_fmt}
                    onchange={on_time_change}
                />
                <button onclick={on_button_click}>
                    { format!("Log {} Dose", &medication.name) }
                </button>
            </div>
        </div>
    }
}
