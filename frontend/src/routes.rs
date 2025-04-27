use yew_router::prelude::*;

#[derive(Clone, Routable, PartialEq)]
pub enum Route {
    #[at("/")]
    Home, // The main patient list page
    #[at("/patients/:id")]
    PatientDetail { id: i64 }, // The detail page for a specific patient
    #[not_found]
    #[at("/404")]
    NotFound, // A catch-all for invalid URLs
}
