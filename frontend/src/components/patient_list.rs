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
    let navigator = use_navigator().unwrap();
    let patient_buttons = patients
        .iter()
        .map(|patient| {
            let patient_route = Route::PatientDetail { id: patient.id };
            let navigator = navigator.clone();

            html! {
                <button onclick={Callback::from(move|_| {navigator.push(&patient_route)})}>
                    { &patient.name }
                </button>
            }
        })
        .collect::<Html>();
    html! { <div class="grid">{ patient_buttons }</div> }
}
