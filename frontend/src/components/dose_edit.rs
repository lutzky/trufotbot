use anyhow::{Result, bail};
use gloo_net::http::Request;
use shared::api::responses;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::{
    error_handling::{self, log_if_error},
    routes::Route,
};

async fn fetch(
    patient_id: i64,
    medication_id: i64,
    dose_id: i64,
) -> Result<responses::GetDoseResponse> {
    let api_url = format!("/api/patients/{patient_id}/doses/{medication_id}/dose/{dose_id}");
    let res = Request::get(&api_url).send().await?;
    if !res.ok() {
        bail!(
            "Failed to fetch medication dose: {} {}",
            res.status(),
            res.status_text()
        );
    }
    Ok(res.json().await?)
}

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
    let response = use_state(|| None::<Result<shared::api::responses::GetDoseResponse>>);

    let fetch_callback = {
        let response = response.clone();
        let patient_id = *patient_id;
        let medication_id = *medication_id;
        let dose_id: i64 = *dose_id;
        Callback::from(move |_: ()| {
            let response = response.clone();

            wasm_bindgen_futures::spawn_local(async move {
                response.set(None);
                let res = fetch(patient_id, medication_id, dose_id).await;
                log_if_error("Failed to fetch medication info:", &res);
                response.set(Some(res));
            });
        })
    };

    use_effect_with((), move |_| {
        fetch_callback.emit(());
    });

    let content = error_handling::error_waiting_or(response.as_ref(), |response| {
        let noted_by_user = response
            .dose
            .data
            .noted_by_user
            .clone()
            .unwrap_or("".to_string());

        html! {
            <>
                <hgroup>
                    <h1>{ format!("Dose {dose_id}") }</h1>
                    <p>{ format!("{} for {}", response.medication_name, response.patient_name) }</p>
                </hgroup>
                <pre>{ format!("{response:#?}") }</pre>
                // TODO remove
                <form>
                    <label for="taken-at">
                        { "Taken at" }
                        <input
                            name="taken-at"
                            aria-label="Taken at"
                            type="datetime-local"
                            placeholder="When was it taken?"
                            // TODO some actual time logic
                            value=""
                        /* TODO *//>
                    </label>
                    <label for="quantity">
                        { "Quantity" }
                        <input
                            name="quantity"
                            aria-label="Quantity"
                            type="number"
                            placeholder="How much of it?"
                            value={format!("{}",response.dose.data.quantity)}
                        />
                    </label>
                    <label for="noted-by">
                        { "Noted by" }
                        <input
                            name="noted-by"
                            aria-label="Noted by"
                            placeholder="Who gave this medication?"
                            type="text"
                            value={noted_by_user.clone()}
                        />
                    </label>
                    <div class="grid">
                        <button role="submit">{ "Submit" }</button>
                        <button role="submit" class="contrast">{ "Delete" }</button>
                    </div>
                </form>
            </>
        }
    });

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
