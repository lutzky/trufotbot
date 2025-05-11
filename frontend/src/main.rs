#![allow(clippy::redundant_closure)]

use yew::prelude::*;
use yew_router::prelude::*;

use components::home::Home;
use components::patient_detail::PatientDetail;
use components::patient_medication_detail::PatientMedicationDetail;

mod components;
mod error_handling;
mod routes;
mod time;

use routes::Route; // Use the Route enum

// --- Router Switch Function ---
fn switch(routes: Route) -> Html {
    match routes {
        Route::Home => html! { <Home /> },
        Route::PatientDetail { id } => html! { <PatientDetail id={id} /> },
        Route::PatientMedicationDetail {
            patient_id,
            medication_id,
        } => {
            html! {
                <PatientMedicationDetail patient_id={patient_id} medication_id={medication_id} />
            }
        }
        Route::NotFound => html! { <h1>{ "404 Not Found" }</h1> },
    }
}

#[function_component(App)]
fn app() -> Html {
    html! {
        <BrowserRouter>
            <main class="container">
                <Switch<Route> render={switch} />
            </main>
        </BrowserRouter>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
