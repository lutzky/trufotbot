use yew::prelude::*;

use anyhow::{Result, bail};
use chrono::TimeZone;

use gloo_console::{error, info, warn};
use gloo_net::http::Request;
use web_sys::HtmlInputElement;

#[derive(Properties, PartialEq)]
pub struct PatientMedicationDetailProps {
    pub patient_id: i64,
    pub medication_id: i64,
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
    let payload = shared::api::dose::CreateDose {
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
        medication_id,
    }: &PatientMedicationDetailProps,
) -> Html {
    // let last_taken = medication
    //     .last_taken_at
    //     .map(|dt| DateTime::<Local>::from(dt).format("%c %z").to_string())
    //     .unwrap_or_else(|| "Never taken".to_string());

    let patient_get_doses_response =
        use_state(|| None::<shared::api::responses::PatientGetDosesResponse>);

    {
        let patient_get_doses_response = patient_get_doses_response.clone();
        let patient_id = *patient_id;
        let medication_id = *medication_id;
        use_effect_with((), move |_| {
            let patient_get_doses_response = patient_get_doses_response.clone();
            let api_url = format!("/api/patients/{}/doses/{}", patient_id, medication_id);

            wasm_bindgen_futures::spawn_local(async move {
                match Request::get(&api_url).send().await {
                    Ok(response) => {
                        if response.ok() {
                            match response
                                .json::<shared::api::responses::PatientGetDosesResponse>()
                                .await
                            {
                                Ok(fetched_doses) => {
                                    info!("Fetched medication doses data");
                                    patient_get_doses_response.set(Some(fetched_doses));
                                }
                                Err(e) => {
                                    error!("Failed to parse medication doses JSON:", e.to_string());
                                }
                            }
                        } else {
                            error!(
                                "Failed to fetch medication doses: Status ",
                                response.status()
                            );
                        }
                    }
                    Err(e) => {
                        error!("Network error fetching medication doses:", e.to_string());
                    }
                }
            });
            || ()
        });
    }

    use std::rc::Rc;

    let time_taken = use_state(|| Rc::new(chrono::Local::now()));
    let time_taken_fmt = format!("{}", time_taken.format("%FT%T"));

    let on_time_change = {
        let time_taken = time_taken.clone();
        Callback::from(move |e: yew::Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            if let Some(t) = try_parse_time_as_local(&input.value()) {
                time_taken.set(Rc::new(t));
            };
        })
    };

    let on_button_click = {
        let patient_id = *patient_id;
        let medication_id = *medication_id;
        let time_taken = time_taken.clone();
        Callback::from(move |_| {
            let time_taken = time_taken.clone();
            wasm_bindgen_futures::spawn_local(async move {
                log_dose(patient_id, medication_id, time_taken.to_utc())
                    .await
                    .unwrap_or_else(|e| {
                        error!(format!("Failed to log dose: {}", e));
                    });
            })
        })
    };

    let content = match patient_get_doses_response.as_ref() {
        None => {
            html! { <p>{"Loading..."}</p> }
        }
        Some(r) => {
            let mut r = r.clone();
            r.doses
                .sort_by(|a, b| b.data.taken_at.cmp(&a.data.taken_at));
            html! {
                <>
                    <hgroup>
                        <h1>{r.medication_name}</h1>
                        <p class="secondary">{r.patient_name}</p>
                    </hgroup>
                    <table>
                        <thead>
                            <tr>
                                <th>{"Taken At"}</th>
                                <th>{"Quantity"}</th>
                            </tr>
                        </thead>
                        <tbody>
                            { r.doses.iter().map(|dose| {
                                let dose = dose.clone();
                                html! {
                                    <tr class="dose-item">
                                        <td>{dose.data.taken_at.with_timezone(&chrono::Local).format("%c %Z").to_string()}</td>
                                        <td>{format!("{}", dose.data.quantity)}</td>
                                    </tr>
                                }
                            }).collect::<Html>() }
                        </tbody>
                    </table>
                </>
            }
        }
    };

    html! {
        <>
        { content }
        <div class="medication-item" key={*medication_id}>
            <h3>{"Medication "}{ *medication_id }</h3>
            <p>{"Last taken: "}{ "good question"}</p>
            // TODO: Add chrono-humanize to show "how long ago" this is
            <div style="display: flex; gap: 8px; align-items: center;">
                <input
                    type="datetime-local"
                    value={time_taken_fmt}
                    onchange={on_time_change}
                />
                <button onclick={on_button_click}>
                    { format!("Log {} Dose", "Oh wow you should really know the medication name") }
                </button>
            </div>
        </div>
        </>
    }
}
