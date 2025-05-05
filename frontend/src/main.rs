#![allow(clippy::redundant_closure)]

use yew::prelude::*;
use yew_router::prelude::*;

use components::home::Home;
use components::patient_detail::PatientDetail;
use components::patient_medication_detail::PatientMedicationDetail;

mod components;
mod routes;

use routes::Route; // Use the Route enum

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
