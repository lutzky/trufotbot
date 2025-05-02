use chrono::TimeZone;
use gloo_console::{error, info};
use gloo_net::http::Request;
use shared::api::patient_types::MedicationMenu;
use web_sys::{HtmlInputElement, wasm_bindgen::JsCast};
use yew::prelude::*;
use yew_router::prelude::*;

mod model;
mod routes;

use routes::Route; // Use the Route enum

// --- Components ---

// Renamed the original App to Home, as it shows the patient list
#[function_component(Home)]
fn home() -> Html {
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

    // Create a function to format current time in local timezone for datetime-local input
    let format_current_time = || {
        let now = chrono::Local::now();
        now.format("%Y-%m-%dT%H:%M").to_string()
    };

    // Create state for each medication's time input
    let time_inputs = use_state(std::collections::HashMap::<i64, String>::new);

    // Create a callback to update time inputs
    let on_time_change = {
        let time_inputs = time_inputs.clone();
        Callback::from(move |e: Event| {
            let target = e.target().unwrap();
            let input = target.dyn_into::<HtmlInputElement>().unwrap();
            let med_id = input
                .get_attribute("data-medication-id")
                .unwrap()
                .parse::<i64>()
                .unwrap();
            let mut new_times = (*time_inputs).clone();
            new_times.insert(med_id, input.value());
            time_inputs.set(new_times);
        })
    };

    // Create a function to fetch medication data
    let fetch_medications = {
        let medication_menu = medication_menu.clone();
        let error_message = error_message.clone();

        Callback::from(move |_| {
            let medication_menu = medication_menu.clone();
            let error_message = error_message.clone();
            let api_url = format!("/api/patients/{}", patient_id);

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
        })
    };

    // Initial fetch on component mount
    {
        let fetch_medications = fetch_medications.clone();
        use_effect_with((), move |_| {
            fetch_medications.emit(());
            || ()
        });
    }

    // Create log_dose callback that will refresh data after logging
    let log_dose_callback = {
        let fetch_medications = fetch_medications.clone();
        let time_inputs = time_inputs.clone();

        Callback::from(move |payload: (i64, i64)| {
            let fetch_medications = fetch_medications.clone();
            let (patient_id, medication_id) = payload;

            // Get the selected time from time_inputs, or use current time as fallback
            let time_str = (*time_inputs)
                .get(&medication_id)
                .cloned()
                .unwrap_or_else(format_current_time);

            // Parse the local time and convert to UTC
            let local_time = chrono::NaiveDateTime::parse_from_str(
                &format!("{}:00", time_str),
                "%Y-%m-%dT%H:%M:%S",
            )
            .unwrap();

            let local_tz = chrono::Local;
            let utc_time = local_tz
                .from_local_datetime(&local_time)
                .earliest() // Get the earliest possible interpretation
                .unwrap()
                .naive_utc();

            wasm_bindgen_futures::spawn_local(async move {
                let api_url = format!("/api/patients/{}/doses/{}", patient_id, medication_id);
                let payload = shared::api::patient_types::CreateDose {
                    quantity: 1.0,
                    taken_at: utc_time,
                    noted_by_user: None,
                };

                match Request::put(&api_url).json(&payload).unwrap().send().await {
                    Ok(response) => {
                        if response.ok() {
                            info!("Dose logged successfully via API.");
                            fetch_medications.emit(()); // Refresh the medication list
                        } else {
                            error!("Failed to log dose: Status ", response.status());
                        }
                    }
                    Err(e) => {
                        error!("Network error logging dose:", e.to_string());
                    }
                }
            });
        })
    };

    // Render based on fetch state
    let content = match ((*medication_menu).clone(), (*error_message).clone()) {
        (_, Some(msg)) => html! { <p style="color: red;">{ msg }</p> },
        (Some(menu), _) => html! {
            <div>
                <h2>{ format!("Medications for {}", &menu.patient_name) }</h2>
                <div class="medications-list">
                    { menu.medications.iter().map(|medication| {
                        let medication = medication.clone();
                        let last_taken = medication.last_taken_at
                            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                            .unwrap_or_else(|| "Never taken".to_string());

                        let current_time = (*time_inputs)
                            .get(&medication.id)
                            .cloned()
                            .unwrap_or_else(format_current_time);

                        html! {
                            <div class="medication-item" key={medication.id}>
                                <h3>{ &medication.name }</h3>
                                <p>{ format!("Last taken: {}", last_taken) }</p>
                                <div style="display: flex; gap: 8px; align-items: center;">
                                    <input
                                        type="datetime-local"
                                        value={current_time}
                                        data-medication-id={medication.id.to_string()}
                                        onchange={on_time_change.clone()}
                                    />
                                    <button onclick={
                                        let log_dose = log_dose_callback.clone();
                                        let patient_id = menu.patient_id;
                                        let medication_id = medication.id;
                                        Callback::from(move |_| log_dose.emit((patient_id, medication_id)))
                                    }>
                                        { format!("Log {} Dose", &medication.name) }
                                    </button>
                                </div>
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
