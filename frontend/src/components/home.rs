use shared::api::patient_types;
use yew::prelude::*;

use crate::components::patient_list::PatientList;
use gloo_console::error;
use gloo_net::http::Request;

#[function_component(Home)]
pub fn home() -> Html {
    let patients = use_state(Vec::new);
    let error_message = use_state(|| None::<String>); // State for error messages

    // Fetch patients on component mount
    {
        let patients = patients.clone();
        let error_message = error_message.clone();
        use_effect_with((), move |_| {
            let patients = patients.clone();
            let error_message = error_message.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match Request::get("/api/patients").send().await {
                    Ok(response) => {
                        if response.ok() {
                            match response.json::<Vec<patient_types::Patient>>().await {
                                Ok(fetched_patients) => {
                                    patients.set(fetched_patients);
                                    error_message.set(None); // Clear error on success
                                }
                                Err(e) => {
                                    error!("Failed to parse patients JSON:", e.to_string());
                                    error_message
                                        .set(Some(format!("Failed to parse patient data: {}", e)));
                                }
                            }
                        } else {
                            error!("Failed to fetch patients: Status ", response.status());
                            error_message.set(Some(format!(
                                "Failed to fetch patients: Server responded with status {}",
                                response.status()
                            )));
                        }
                    }
                    Err(e) => {
                        error!("Network error fetching patients:", e.to_string());
                        error_message.set(Some(format!("Network error: {}", e)));
                    }
                }
            });
            || () // Cleanup function (optional)
        });
    }

    html! {
        <div> // Wrap content
            <h1>{ "Select Patient" }</h1>
            if let Some(msg) = &*error_message {
                <p style="color: red;">{ msg }</p>
            }
            if patients.is_empty() && error_message.is_none() {
                <p>{ "Loading patients..." }</p>
            } else {
                <PatientList patients={(*patients).clone()} />
            }
        </div>
    }
}
