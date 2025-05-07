use yew::prelude::*;
use yew_router::prelude::*;

use crate::routes::Route;
use shared::api::patient;

#[derive(Properties, PartialEq)]
pub struct PatientListProps {
    pub patients: Vec<patient::Patient>,
}

#[function_component(PatientList)]
pub fn patient_list(PatientListProps { patients }: &PatientListProps) -> Html {
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
