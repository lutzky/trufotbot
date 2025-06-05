use yew::prelude::*;
use yew_router::prelude::*;

use gloo_net::http::Request;

use crate::{
    Route,
    components::{
        medication_edit::{MedicationEdit, MedicationEditMode},
        patient_settings::PatientSettings,
    },
    error_handling::{self, log_if_error},
    time::humanize_html,
};

use anyhow::{Result, bail};
use shared::{
    api::{
        dose::AvailableDose,
        medication::MedicationSummary,
        requests::PatientCreateRequest,
        responses::{self, PatientGetResponse},
    },
    time::{future_time, now},
};

async fn api_delete(patient_id: i64) -> Result<()> {
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

async fn api_fetch(patient_id: i64) -> Result<responses::PatientGetResponse> {
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

async fn api_update_settings(patient_id: i64, req: &PatientCreateRequest) -> Result<()> {
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
    let can_take = props
        .medication_summary
        .next_doses
        .iter()
        .map(|dose| AvailableDose {
            time: dose.time.max(now()),
            quantity: dose.quantity,
        })
        .collect::<Vec<_>>();
    let can_take = can_take
        .iter()
        .map(|dose| {
            let amount = dose.quantity.map(|q| q.to_string()).unwrap_or_default();
            let time = future_time(&dose.time);
            format!("{amount} {time}")
        })
        .collect::<Vec<_>>()
        .join(", or ");

    let navigator = use_navigator().unwrap();

    let navigate_to_medication = Callback::from(move |_| {
        navigator.push(&medication_route);
    });

    html! {
        <article style="cursor: pointer" onclick={navigate_to_medication}>
            <h2>{ &medication.name }{ " ›" }</h2>
            <footer>
            <p>{ "Can take " }{ can_take }{ "." }</p>
            <p>{ "Last taken: " }{ last_taken }</p></footer>
        </article>
    }
}

type ResponseState = UseStateHandle<Option<Result<PatientGetResponse>>>;

fn make_fetch_callback(response: ResponseState, patient_id: i64) -> Callback<()> {
    Callback::from(move |_: ()| {
        let response = response.clone();

        wasm_bindgen_futures::spawn_local(async move {
            response.set(None);
            let res = api_fetch(patient_id).await;
            log_if_error("Failed to fetch patient details", &res);
            response.set(Some(res));
        });
    })
}

fn make_save_settings_callback(
    fetch_callback: Callback<()>,
    patient_id: i64,
) -> Callback<PatientCreateRequest> {
    Callback::from(move |req: PatientCreateRequest| {
        let fetch_callback = fetch_callback.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let res = api_update_settings(patient_id, &req).await;
            log_if_error("Failed to update patient settings", &res);
            if res.is_ok() {
                fetch_callback.emit(());
            }
        });
    })
}

fn make_delete_callback(navigator: Navigator, patient_id: i64) -> Callback<MouseEvent> {
    Callback::from(move |_: MouseEvent| {
        let navigator = navigator.clone();
        wasm_bindgen_futures::spawn_local(async move {
            if !gloo_dialogs::confirm("Are you sure you want to delete this patient?") {
                return;
            }
            let res = api_delete(patient_id).await;
            log_if_error("Failed to update patient settings", &res);
            if res.is_ok() {
                navigator.push(&Route::Home);
            }
        });
    })
}

#[derive(Properties, PartialEq)]
pub struct PatientDetailProps {
    pub id: i64,
}

#[function_component(PatientDetail)]
pub fn patient_detail(PatientDetailProps { id: patient_id }: &PatientDetailProps) -> Html {
    let response = use_state(|| None::<Result<responses::PatientGetResponse>>);

    let delete_callback = make_delete_callback(use_navigator().unwrap(), *patient_id);
    let fetch_callback = make_fetch_callback(response.clone(), *patient_id);
    let save_settings_callback = make_save_settings_callback(fetch_callback.clone(), *patient_id);

    {
        let fetch_callback = fetch_callback.clone();
        use_effect_with((), move |_| fetch_callback.emit(()));
    }

    let content = error_handling::error_waiting_or(response.as_ref(), move |response| {
        render_content(
            response,
            *patient_id,
            delete_callback.clone(),
            save_settings_callback.clone(),
        )
    });

    let new_medication = html! {
        <details>
            <summary>{ "Add new medication" }</summary>
            <MedicationEdit mode={MedicationEditMode::Create} onsave={fetch_callback} />
        </details>
    };

    html! {
        <>
            <Link<Route> classes="secondary" to={Route::Home}>
                { "< Back to Patient List" }
            </Link<Route>>
            { content }
            <hr />
            { new_medication }
        </>
    }
}

fn render_content(
    response: &PatientGetResponse,
    patient_id: i64,
    delete_callback: Callback<MouseEvent>,
    save_settings_callback: Callback<PatientCreateRequest>,
) -> Html {
    let (taken, never_taken): (Vec<_>, Vec<_>) = response
        .medications
        .iter()
        .partition(|med| med.last_taken_at.is_some());

    let summary_vec = |medications: &Vec<&MedicationSummary>| -> Html {
        medications
            .iter()
            .map(|med| {
                html! {
                    <PatientMedicationSummaryCard
                        patient_id={patient_id}
                        medication_summary={(*med).clone()}
                    />
                }
            })
            .collect::<Html>()
    };

    let title = format!("Medications for {}", &response.name);

    html! {
        <>
            <h1>{ title }</h1>
            { summary_vec(&taken) }
            if !taken.is_empty() && !never_taken.is_empty() {
                <hr />
            }
            { summary_vec(&never_taken) }
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
}
