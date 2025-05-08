use yew::prelude::*;
use yew_router::prelude::*;

use gloo_console::{error, info};
use gloo_net::http::Request;

use crate::Route;

use shared::api::responses;

#[derive(Properties, PartialEq)]
pub struct PatientDetailProps {
    pub id: i64, // Received from the router
}

#[function_component(PatientDetail)]
pub fn patient_detail(props: &PatientDetailProps) -> Html {
    let patient_id = props.id;
    let patient_get_response = use_state(|| None::<responses::PatientGetResponse>);
    let error_message = use_state(|| None::<String>);

    // TODO(lutzky): Simplify

    // Create a function to fetch medication data
    let fetch_medications = {
        let patient_get_response = patient_get_response.clone();
        let error_message = error_message.clone();

        Callback::from(move |_: ()| {
            let patient_get_response = patient_get_response.clone();
            let error_message = error_message.clone();
            let api_url = format!("/api/patients/{}", patient_id);

            wasm_bindgen_futures::spawn_local(async move {
                match Request::get(&api_url).send().await {
                    Ok(response) => {
                        if response.ok() {
                            match response.json::<responses::PatientGetResponse>().await {
                                Ok(fetched_menu) => {
                                    info!("Fetched medication menu data");
                                    patient_get_response.set(Some(fetched_menu));
                                    error_message.set(None);
                                }
                                Err(e) => {
                                    error!("Failed to parse medication menu JSON:", e.to_string());
                                    error_message
                                        .set(Some(format!("Error parsing medication data: {}", e)));
                                }
                            }
                        } else {
                            error!(
                                "Failed to fetch medication menu: Status ",
                                response.status()
                            );
                            error_message.set(Some(format!(
                                "Error fetching medication data: Server responded with status {}",
                                response.status()
                            )));
                        }
                    }
                    Err(e) => {
                        error!("Network error fetching medication menu:", e.to_string());
                        error_message.set(Some(format!("Network error: {}", e)));
                    }
                }
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

    let refresh_medications_callback = {
        let fetch_medications = fetch_medications.clone();

        Callback::from(move |_: ()| {
            let fetch_medications = fetch_medications.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let fetch_medications = fetch_medications.clone();
                fetch_medications.emit(()); // Refresh the medication list
            });
        })
    };

    // Render based on fetch state
    let content = match ((*patient_get_response).clone(), (*error_message).clone()) {
        (_, Some(msg)) => html! { <p style="color: red;">{ msg }</p> },
        (Some(response), _) => html! {
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
                        // html! {
                        //     <PatientMedicationDetail patient_id={patient_id}
                        //      medication={medication} on_log_dose={refresh_medications_callback.clone()}/>
                        // }
                    }).collect::<Html>() }
                </div>
            </div>
        },
        (None, None) => html! { <p>{ "Loading medications..." }</p> },
    };

    html! {
        <div>
            // Optional: Add a link back to the home page
            <Link<Route> to={Route::Home}>{ "< Back to Patient List" }</Link<Route>>
            { content }
        </div>
    }
}
