use yew::prelude::*;
use yew_router::prelude::*;

use gloo_net::http::Request;

use crate::{
    Route,
    error_handling::{self, log_if_error},
};

use anyhow::{Result, bail};
use shared::api::responses;

#[derive(Properties, PartialEq)]
pub struct PatientDetailProps {
    pub id: i64, // Received from the router
}

async fn fetch(patient_id: i64) -> Result<responses::PatientGetResponse> {
    let api_url = format!("/api/patients/{}", patient_id);
    let res = Request::get(&api_url).send().await?;
    if !res.ok() {
        bail!(
            "Fetching patient details returned non-OK response: {} {}",
            res.status(),
            res.status_text()
        );
    }
    Ok(res.json().await?)
}

#[function_component(PatientDetail)]
pub fn patient_detail(props: &PatientDetailProps) -> Html {
    let patient_id = props.id;
    let patient_get_response = use_state(|| None::<Result<responses::PatientGetResponse>>);

    // TODO(lutzky): Simplify

    // Create a function to fetch medication data
    let fetch_medications = {
        let patient_get_response = patient_get_response.clone();

        Callback::from(move |_: ()| {
            let patient_get_response = patient_get_response.clone();

            wasm_bindgen_futures::spawn_local(async move {
                patient_get_response.set(None);
                let res = fetch(patient_id).await;
                log_if_error("Failed to fetch patient details", &res);
                patient_get_response.set(Some(res));
            });
        })
    };

    // Initial fetch on component mount
    // TODO(lutzky): Understand why this works, what the connection is between this and refresh_medications_callback
    {
        let fetch_medications = fetch_medications.clone();
        use_effect_with((), move |_| {
            fetch_medications.emit(());
            || ()
        });
    }

    // Render based on fetch state
    let content = error_handling::error_waiting_or(
        patient_get_response.as_ref(),
        move |response| {
            html! {
                <div>
                    <h2>{ format!("Medications for {}", &response.patient_name) }</h2>
                    <div class="medications-list">
                        // These should show last-taken, humanized, and be sorted by that
                        { response.medications.iter().map(|medication| {
                            let medication = medication.clone();
                            let medication_route = Route::PatientMedicationDetail { patient_id, medication_id: medication.id };

                            html!{
                                <Link<Route> to={medication_route} classes="patient-link"> // Add a class for styling
                                    <div class="patient" style="border: 1px solid black; padding: 10px; margin-bottom: 10px; cursor: pointer;">
                                        <h1>{ &medication.name }</h1>
                                        // <p>{ "Patient details go here." }</p> // Removed redundant text
                                        // The button below is now just an example, navigation is the main action
                                        // <button onclick={ping_them}>{ "Ping" }</button>
                                    </div>
                                </Link<Route>>
                            }
                        }).collect::<Html>() }
                    </div>
                </div>
            }
        },
    );

    html! {
        <div>
            // TODO: Add back-links everywhere
            <Link<Route> classes="secondary" to={Route::Home}>{ "< Back to Patient List" }</Link<Route>>
            { content }
        </div>
    }
}
