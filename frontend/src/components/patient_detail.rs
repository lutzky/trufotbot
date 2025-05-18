use yew::prelude::*;
use yew_router::prelude::*;

use gloo_net::http::Request;

use crate::{
    Route,
    components::patient_settings::PatientSettings,
    error_handling::{self, log_if_error},
    time::humanize_html,
};

use anyhow::{Result, bail};
use shared::api::{medication::MedicationSummary, requests::PatientCreateRequest, responses};

async fn delete(patient_id: i64) -> Result<()> {
    let api_url = format!("/api/patients/{}", patient_id);
    let res = Request::delete(&api_url).send().await?;
    if !res.ok() {
        bail!(
            "Deleting patient returned non-OK response: {} {}",
            res.status(),
            res.status_text()
        );
    }
    Ok(())
}

async fn fetch(patient_id: i64) -> Result<responses::PatientGetResponse> {
    let api_url = format!("/api/patients/{}", patient_id);
    let res = Request::get(&api_url).send().await?;
    if !res.ok() {
        bail!(
            "Fetching patient details returned non-OK response: {} {}",
            res.status(),
            res.status_text()
        );
    }
    Ok(res.json().await?)
}

async fn update_settings(patient_id: i64, req: &PatientCreateRequest) -> Result<()> {
    let api_url = format!("/api/patients/{}", patient_id);
    let res = Request::put(&api_url).json(req)?.send().await?;
    if !res.ok() {
        bail!(
            "Updating patient details returned non-OK response: {} {}",
            res.status(),
            res.status_text()
        );
    }
    Ok(())
}

#[derive(Properties, PartialEq)]
struct PatientMedicationSummaryCardProps {
    patient_id: i64,
    medication_summary: MedicationSummary,
}

#[function_component(PatientMedicationSummaryCard)]
fn patient_medication_summary_card(props: &PatientMedicationSummaryCardProps) -> Html {
    let medication = &props.medication_summary;
    let medication_route = Route::PatientMedicationDetail {
        patient_id: props.patient_id,
        medication_id: medication.id,
    };
    let last_taken = match medication.last_taken_at {
        None => html! { "Never" },
        Some(last_taken) => humanize_html(&last_taken),
    };

    let navigator = use_navigator().unwrap();

    let navigate_to_medication = Callback::from(move |_| {
        navigator.push(&medication_route);
    });

    html! {
        <article style="cursor: pointer" onclick={navigate_to_medication}>
            <h2>{ &medication.name }{ " ›" }</h2>
            <p>{ "More stuff" }</p>
            <footer>{ "Last taken: " }{ last_taken }</footer>
        </article>
    }
}

#[derive(Properties, PartialEq)]
pub struct PatientDetailProps {
    pub id: i64, // Received from the router
}

#[function_component(PatientDetail)]
pub fn patient_detail(props: &PatientDetailProps) -> Html {
    let patient_id = props.id;
    let patient_get_response = use_state(|| None::<Result<responses::PatientGetResponse>>);

    let fetch_callback = {
        let patient_get_response = patient_get_response.clone();
        Callback::from(move |_: ()| {
            let patient_get_response = patient_get_response.clone();

            wasm_bindgen_futures::spawn_local(async move {
                patient_get_response.set(None);
                let res = fetch(patient_id).await;
                log_if_error("Failed to fetch patient details", &res);
                patient_get_response.set(Some(res));
            });
        })
    };

    {
        let fetch_callback = fetch_callback.clone();
        use_effect_with((), move |_| fetch_callback.emit(()));
    }

    let save_settings_callback = {
        let fetch_callback = fetch_callback.clone();
        Callback::from(move |req: PatientCreateRequest| {
            let fetch_callback = fetch_callback.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let res = update_settings(patient_id, &req).await;
                log_if_error("Failed to update patient settings", &res);
                if res.is_ok() {
                    fetch_callback.emit(());
                }
            });
        })
    };

    let delete_callback = {
        let navigator = use_navigator().unwrap();
        Callback::from(move |_| {
            let navigator = navigator.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if !gloo_dialogs::confirm("Are you sure you want to delete this patient?") {
                    return;
                }
                let res = delete(patient_id).await;
                log_if_error("Failed to update patient settings", &res);
                if res.is_ok() {
                    navigator.push(&Route::Home);
                }
            });
        })
    };

    let content =
        error_handling::error_waiting_or(patient_get_response.as_ref(), move |response| {
            let (taken, never_taken): (Vec<_>, Vec<_>) = response
                .medications
                .iter()
                .partition(|med| med.last_taken_at.is_some());
            html! {
                <>
                    <h1>{ format!("Medications for {}", &response.name) }</h1>
                    // These should show last-taken, humanized, and be sorted by that
                    { taken.iter().map(|&medication| {
                    html! {
                        <PatientMedicationSummaryCard
                            patient_id={patient_id}
                            medication_summary={medication.clone()}/>
                    }
                }).collect::<Html>() }
                    <hr />
                    { never_taken.iter().map(|&medication| {
                    html! {
                        <PatientMedicationSummaryCard
                            patient_id={patient_id}
                            medication_summary={medication.clone()}/>
                    }
                }).collect::<Html>() }
                    <hr />
                    <details>
                        <summary>{ "Edit patient" }</summary>
                        <PatientSettings
                            group=false
                            name={response.name.clone()}
                            telegram_group_id={response.telegram_group_id}
                            onsave={save_settings_callback.clone()}
                        />
                        <div class="grid">
                            <button onclick={delete_callback.clone()} class="contrast">
                                { "Delete Patient" }
                            </button>
                        </div>
                    </details>
                </>
            }
        });

    html! {
        <>
            <Link<Route> classes="secondary" to={Route::Home}>
                { "< Back to Patient List" }
            </Link<Route>>
            { content }
        </>
    }
}
