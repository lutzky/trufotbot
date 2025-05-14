use anyhow::{Result, bail};
use gloo_net::http::Request;
use shared::api::{
    dose::{CreateDose, Dose},
    responses::{self, GetDoseResponse},
};
use web_sys::HtmlInputElement;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::{
    error_handling::{self, log_if_error},
    routes::Route,
    time::LocalTime,
};

async fn fetch(
    patient_id: i64,
    medication_id: i64,
    dose_id: i64,
) -> Result<responses::GetDoseResponse> {
    let api_url = format!("/api/patients/{patient_id}/doses/{medication_id}/dose/{dose_id}");
    let res = Request::get(&api_url).send().await?;
    if !res.ok() {
        bail!(
            "Failed to fetch medication dose: {} {}",
            res.status(),
            res.status_text()
        );
    }
    Ok(res.json().await?)
}

#[derive(Properties, PartialEq)]
pub struct DoseEditProps {
    pub patient_id: i64,
    pub medication_id: i64,
    pub dose_id: i64,
}

#[function_component(DoseEdit)]
pub fn dose_edit(
    DoseEditProps {
        patient_id,
        medication_id,
        dose_id,
    }: &DoseEditProps,
) -> Html {
    let response = use_state(|| None::<Result<shared::api::responses::GetDoseResponse>>);

    let fetch_callback = {
        let response = response.clone();
        let patient_id = *patient_id;
        let medication_id = *medication_id;
        let dose_id: i64 = *dose_id;
        Callback::from(move |_: ()| {
            let response = response.clone();

            wasm_bindgen_futures::spawn_local(async move {
                response.set(None);
                let res = fetch(patient_id, medication_id, dose_id).await;
                log_if_error("Failed to fetch medication info:", &res);
                response.set(Some(res));
            });
        })
    };

    let set_time = {
        // TODO: Break up the response god-object
        let response = response.clone();
        Callback::from(move |t: chrono::DateTime<chrono::Utc>| {
            let Some(Ok(current_response)) = &(*response) else {
                return;
            };
            let current_response = current_response.clone();

            response.set(Some(Ok(GetDoseResponse {
                dose: Dose {
                    data: CreateDose {
                        taken_at: t,
                        ..current_response.dose.data
                    },
                    ..current_response.dose
                },
                ..current_response
            })));
        })
    };

    let set_noted_by = {
        // TODO: Break up the response god-object
        let response = response.clone();
        Callback::from(move |e: Event| {
            let Some(Ok(current_response)) = &(*response) else {
                return;
            };
            let current_response = current_response.clone();

            let noted_by_user = match e.target_unchecked_into::<HtmlInputElement>().value() {
                s if s.is_empty() => None,
                s => Some(s),
            };

            response.set(Some(Ok(GetDoseResponse {
                dose: Dose {
                    data: CreateDose {
                        noted_by_user,
                        ..current_response.dose.data
                    },
                    ..current_response.dose
                },
                ..current_response
            })));
        })
    };

    let set_quantity = {
        // TODO: Break up the response god-object
        let response = response.clone();
        Callback::from(move |e: Event| {
            let Some(Ok(current_response)) = &(*response) else {
                return;
            };
            let current_response = current_response.clone();

            let quantity = e
                .target_unchecked_into::<HtmlInputElement>()
                .value()
                .parse()
                .unwrap();

            response.set(Some(Ok(GetDoseResponse {
                dose: Dose {
                    data: CreateDose {
                        quantity,
                        ..current_response.dose.data
                    },
                    ..current_response.dose
                },
                ..current_response
            })));
        })
    };

    use_effect_with((), move |_| {
        fetch_callback.emit(());
    });

    let content = error_handling::error_waiting_or(response.as_ref(), |response| {
        let noted_by_user = response
            .dose
            .data
            .noted_by_user
            .clone()
            .unwrap_or("".to_string());
        let set_time = set_time.clone();
        let set_noted_by = set_noted_by.clone();
        let set_quantity = set_quantity.clone();

        html! {
            <>
                <hgroup>
                    <h1>{ format!("Dose {dose_id}") }</h1>
                    <p>{ format!("{} for {}", response.medication_name, response.patient_name) }</p>
                </hgroup>
                <pre>{ format!("{response:#?}") }</pre>
                // TODO remove
                <form>
                    <label for="taken-at">
                        { "Taken at" }
                        <LocalTime onchange={set_time} utc_time={response.dose.data.taken_at} />
                    </label>
                    <label for="quantity">
                        { "Quantity" }
                        <input
                            name="quantity"
                            onchange={set_quantity}
                            aria-label="Quantity"
                            type="number"
                            placeholder="How much of it?"
                            value={format!("{}",response.dose.data.quantity)}
                        />
                    </label>
                    <label for="noted-by">
                        { "Noted by" }
                        <input
                            name="noted-by"
                            onchange={set_noted_by}
                            aria-label="Noted by"
                            placeholder="Who gave this medication?"
                            type="text"
                            value={noted_by_user.clone()}
                        />
                    </label>
                    <div class="grid">
                        <button role="submit">{ "Submit" }</button>
                        <button role="submit" class="contrast">{ "Delete" }</button>
                    </div>
                </form>
            </>
        }
    });

    html! {
        <>
            <Link<Route>
                classes="secondary"
                to={Route::PatientMedicationDetail{patient_id:*patient_id,medication_id:*medication_id}}
            >
                { "< Back to medication details" }
            </Link<Route>>
            { content }
        </>
    }
}
