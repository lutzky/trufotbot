use yew::prelude::*;
use yew_router::prelude::*;

use crate::routes::Route;

#[derive(Properties, PartialEq)]
pub struct DoseEditProps {
    pub patient_id: i64,
    pub medication_id: i64,
    pub dose_id: i64,
}

#[function_component(DoseEdit)]
pub fn dose_edit(
    DoseEditProps {
        patient_id,
        medication_id,
        dose_id,
    }: &DoseEditProps,
) -> Html {
    let content = html! {
        <>
            <h1>{ "Dose" }</h1>
            { format!("dose edit page for {patient_id}/{medication_id}/{dose_id}") }
            // TODO: Add editing stuff
        </>
    };
    html! {
        <>
            <Link<Route>
                classes="secondary"
                to={Route::PatientMedicationDetail{patient_id:*patient_id,medication_id:*medication_id}}
            >
                { "< Back to medication details" }
            </Link<Route>>
            { content }
        </>
    }
}
