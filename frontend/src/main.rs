use gloo_console::{error, info};
use gloo_net::http::Request;
use shared::api::patient_types::MedicationMenu;
use yew::prelude::*;
use yew_router::prelude::*;

mod model;
mod routes;

use routes::Route; // Use the Route enum

// --- Components ---

// Renamed the original App to Home, as it shows the patient list
#[function_component(Home)]
fn home() -> Html {
    let patients = use_state(|| vec![]);
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
                            match response.json::<Vec<model::Patient>>().await {
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

#[derive(Properties, PartialEq)]
struct PatientListProps {
    patients: Vec<model::Patient>,
}

#[function_component(PatientList)]
fn patient_list(PatientListProps { patients }: &PatientListProps) -> Html {
    patients
        .iter()
        .map(|patient| {
            // No need for the ping callback here anymore, navigation handles the action
            // let ping_them = ...

            // Define the route for this specific patient
            let patient_route = Route::PatientDetail { id: patient.id };

            html! {
                // Use Link component for navigation
                <Link<Route> to={patient_route} classes="patient-link"> // Add a class for styling
                    <div class="patient" style="border: 1px solid black; padding: 10px; margin-bottom: 10px; cursor: pointer;">
                        <h1>{ &patient.name }</h1>
                        // <p>{ "Patient details go here." }</p> // Removed redundant text
                        // The button below is now just an example, navigation is the main action
                        // <button onclick={ping_them}>{ "Ping" }</button>
                    </div>
                </Link<Route>>
            }
        })
        .collect()
}

// --- NEW: Patient Detail Component ---
#[derive(Properties, PartialEq)]
struct PatientDetailProps {
    id: i64, // Received from the router
}

#[function_component(PatientDetail)]
fn patient_detail(props: &PatientDetailProps) -> Html {
    let patient_id = props.id;
    let medication_menu = use_state(|| None::<MedicationMenu>);
    let error_message = use_state(|| None::<String>);

    // Fetch medication menu data when the component mounts or id changes
    {
        let medication_menu = medication_menu.clone();
        let error_message = error_message.clone();
        use_effect_with(patient_id, move |_| {
            let medication_menu = medication_menu.clone();
            let error_message = error_message.clone();
            let api_url = format!("/api/patients/{}", patient_id);

            info!("Fetching medication menu for patient ID:", patient_id);

            wasm_bindgen_futures::spawn_local(async move {
                match Request::get(&api_url).send().await {
                    Ok(response) => {
                        if response.ok() {
                            match response.json::<MedicationMenu>().await {
                                Ok(fetched_menu) => {
                                    info!("Fetched medication menu data");
                                    medication_menu.set(Some(fetched_menu));
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
            || ()
        });
    }

    // Render based on fetch state
    let content = match ((*medication_menu).clone(), (*error_message).clone()) {
        (_, Some(msg)) => html! { <p style="color: red;">{ msg }</p> },
        (Some(menu), _) => html! {
            <div>
                <h2>{ format!("Medications for {}", &menu.patient_name) }</h2>
                <div class="medications-list">
                    { menu.medications.iter().map(|medication| {
                        let medication = medication.clone();  // Clone here to avoid lifetime issues
                        let last_taken = medication.last_taken_at
                            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                            .unwrap_or_else(|| "Never taken".to_string());

                        html! {
                            <div class="medication-item" key={medication.id}>
                                <h3>{ &medication.name }</h3>
                                <p>{ format!("Last taken: {}", last_taken) }</p>
                                <button onclick={
                                    let patient_id = menu.patient_id;
                                    let medication_id = medication.id;
                                    Callback::from(move |_| log_dose(patient_id, medication_id))
                                }>
                                    { format!("Log {} Dose", &medication.name) }
                                </button>
                            </div>
                        }
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

// --- Placeholder Function for Logging Dose ---
// This needs proper implementation with API call, state update etc.
fn log_dose(patient_id: i64, medication_id: i64) {
    info!(format!(
        "Attempting to log dose for patient {} medication {}",
        patient_id, medication_id
    ));
    wasm_bindgen_futures::spawn_local(async move {
        let api_url = format!("/api/patients/{}/doses/{}", patient_id, medication_id);
        let payload = shared::api::patient_types::CreateDose {
            quantity: 1.0, // Default to 1 for now
            taken_at: chrono::Utc::now().naive_utc(),
            noted_by_user: None, // Could add a form later to capture this
        };

        match Request::put(&api_url).json(&payload).unwrap().send().await {
            Ok(response) => {
                if response.ok() {
                    info!("Dose logged successfully via API.");
                    // Trigger a re-fetch of the medication menu to update the UI
                    // TODO: Implement proper state management to trigger re-fetch
                } else {
                    error!("Failed to log dose: Status ", response.status());
                    // TODO: Show error to user in UI
                }
            }
            Err(e) => {
                error!("Network error logging dose:", e.to_string());
                // TODO: Show error to user in UI
            }
        }
    });
}

// --- Router Switch Function ---
fn switch(routes: Route) -> Html {
    match routes {
        Route::Home => html! { <Home /> },
        Route::PatientDetail { id } => html! { <PatientDetail id={id} /> },
        Route::NotFound => html! { <h1>{ "404 Not Found" }</h1> },
    }
}

// --- Main App Component (now uses the router) ---
#[function_component(App)]
fn app() -> Html {
    html! {
        // Use HashRouter for static server compatibility without configuration
        // Use BrowserRouter if your server is configured to handle SPA routing
        <HashRouter>
            <main class="container"> // Keep your container
                <Switch<Route> render={switch} /> // The Switch renders the correct component based on the route
            </main>
        </HashRouter>
    }
}

// --- Main Entry Point ---
fn main() {
    yew::Renderer::<App>::new().render();
}
