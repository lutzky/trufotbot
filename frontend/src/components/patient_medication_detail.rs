use crate::components::{
    dose::Dose,
    medication_edit::{MedicationEdit, MedicationEditMode},
};
use shared::api::{
    dose::CreateDose, requests::CreateDoseQueryParams, responses::PatientGetDosesResponse,
};
use yew::prelude::*;

use anyhow::{Result, bail};
use chrono::{DurationRound, TimeDelta};

use gloo_console::{error, info};
use gloo_net::http::Request;
use yew_router::{
    hooks::{use_location, use_navigator},
    prelude::Link,
};

use crate::{
    error_handling::{self, log_if_error},
    routes::Route,
    time::humanize_html,
    username,
};

#[derive(Properties, PartialEq)]
pub struct PatientMedicationDetailProps {
    pub patient_id: i64,
    pub medication_id: i64,
}

async fn api_log_dose(
    patient_id: i64,
    medication_id: i64,
    utc_time: chrono::DateTime<chrono::Utc>,
    quantity: f64,
    reminder_message_id: Option<i32>,
) -> Result<()> {
    let params = CreateDoseQueryParams {
        reminder_message_id,
    };

    let api_url = format!(
        "/api/patients/{patient_id}/medications/{medication_id}/doses?{}",
        serde_url_params::to_string(&params).unwrap()
    );

    info!("Logging dose to", &api_url);
    info!(format!("Logging dose with utc_time {utc_time:?}"));
    let payload = shared::api::dose::CreateDose {
        quantity,
        taken_at: utc_time,
        noted_by_user: username::get(),
    };

    let response = Request::post(api_url.as_ref())
        .json(&payload)?
        .send()
        .await
        .inspect_err(|e| {
            error!(format!("{e:?}"));
        })?;

    if response.ok() {
        info!("Dose logged successfully via API.");
        Ok(())
    } else {
        bail!("Failed to log dose: Status {}", response.status());
    }
}

async fn api_fetch(patient_id: i64, medication_id: i64) -> Result<PatientGetDosesResponse> {
    let api_url = format!("/api/patients/{patient_id}/medications/{medication_id}/doses");
    let res = Request::get(&api_url).send().await?;
    if !res.ok() {
        bail!(
            "Failed to fetch medication doses: {} {}",
            res.status(),
            res.status_text()
        );
    }
    Ok(res.json().await?)
}

fn doses_table(patient_id: i64, medication_id: i64, r: &PatientGetDosesResponse) -> Html {
    html! {
        <table>
            <thead>
                <tr>
                    <th>{ "Time taken" }</th>
                    <th>{ "Quantity" }</th>
                    <th />
                </tr>
            </thead>
            <tbody>
                { r.doses.iter().map(|dose| {
                    let dose = dose.clone();
                    html! {
                        <tr class="dose-item">
                            <td>{humanize_html(&dose.data.taken_at)}</td>
                            <td>{format!("{}", dose.data.quantity)}</td>
                            <td style="text-align: right">
                              <Link<Route> classes="secondary" to={Route::DoseEdit{patient_id,medication_id,dose_id:dose.id}}>
                                <span class="material-symbols-rounded">{ "edit" }</span>
                              </Link<Route>>
                            </td>
                        </tr>
                    }
                }).collect::<Html>() }
            </tbody>
        </table>
    }
}

#[derive(serde::Deserialize, Debug)]
struct QueryParams {
    pub message_id: Option<i32>,
}

#[function_component(PatientMedicationDetail)]
pub fn patient_medication_detail(
    PatientMedicationDetailProps {
        patient_id,
        medication_id,
    }: &PatientMedicationDetailProps,
) -> Html {
    let response = use_state(|| None::<Result<PatientGetDosesResponse>>);

    let quantity = use_state(|| 1.0);
    let fetch_callback = make_fetch_callback(
        response.clone(),
        quantity.clone(),
        *patient_id,
        *medication_id,
    );

    {
        let fetch_callback = fetch_callback.clone();
        use_effect_with((), move |_| {
            fetch_callback.emit(());
        });
    }

    let time_taken = use_state(|| {
        // Round time down so we never log a dose that's "in the future"
        chrono::Utc::now()
            .duration_trunc(TimeDelta::minutes(1))
            .unwrap()
    });

    let reminder_message_id: Option<i32> = use_location()
        .and_then(|l| {
            l.query::<QueryParams>()
                .inspect_err(|e| error!("Failed to fetch query params:", e.to_string()))
                .ok()
        })
        .and_then(|params| params.message_id);

    let button_click_callback = make_button_click_callback(
        *patient_id,
        *medication_id,
        reminder_message_id,
        fetch_callback.clone(),
        *time_taken,
        *quantity,
    );

    let dose_data = CreateDose {
        quantity: *quantity,
        taken_at: *time_taken,
        noted_by_user: None, // unused
    };

    let update_data_callback = {
        let time_taken = time_taken.clone();
        Callback::from(move |data: CreateDose| {
            time_taken.set(data.taken_at);
            quantity.set(data.quantity);
        })
    };

    let skipped_dose_hint = match reminder_message_id {
        Some(_) => concat!(
            r#"Note: To mark this as a "skipped" dose, set "#,
            r#"the quantity to 0."#
        ),
        None => "",
    };

    let log_dose_form = html! {
        <>
            <fieldset role="group">
                <Dose data={dose_data} oninput={update_data_callback} show_noted_by=false />
                <input onclick={button_click_callback} type="submit" value="Log dose" />
            </fieldset>
            <small>{ skipped_dose_hint }</small>
        </>
    };

    let navigator = use_navigator().expect("Navigator should be available");
    let back_route = Route::PatientDetail { id: *patient_id };

    let medication_delete_callback = {
        let navigator = navigator.clone();
        let back_route = back_route.clone();
        Callback::from(move |_: ()| navigator.push(&back_route))
    };

    let medication_save_callback = {
        let fetch_callback = fetch_callback.clone();
        Callback::from(move |_: ()| fetch_callback.emit(()))
    };

    let content = error_handling::error_waiting_or(response.as_ref(), move |r| {
        render_content(
            *patient_id,
            *medication_id,
            r,
            log_dose_form.clone(),
            medication_delete_callback.clone(),
            medication_save_callback.clone(),
        )
    });

    let patient_name = match response.as_ref() {
        None | Some(Err(_)) => "Patient",
        Some(Ok(r)) => &r.patient_name,
    };

    html! {
        <>
            <Link<Route> classes="secondary" to={back_route}>
                { "< Back to " }
                { patient_name }
            </Link<Route>>
            { content }
        </>
    }
}

fn render_content(
    patient_id: i64,
    medication_id: i64,
    response: &PatientGetDosesResponse,
    log_dose_form: Html,
    medication_delete_callback: Callback<()>,
    medication_save_callback: Callback<()>,
) -> Html {
    let log_dose_form = log_dose_form.clone();

    html! {
        <>
            <hgroup>
                <h1>{ &response.medication_name }</h1>
                if let Some(desc) = &response.medication_description {
                    <p>{ desc }</p>
                }
            </hgroup>
            { log_dose_form }
            { doses_table(patient_id, medication_id, response) }
            <details>
                <summary>{ "Edit medication" }</summary>
                <MedicationEdit
                    mode={MedicationEditMode::Edit(patient_id, medication_id)}
                    name={response.medication_name.clone()}
                    description={response.medication_description.clone()}
                    onsave={medication_save_callback}
                    ondelete={medication_delete_callback}
                />
            </details>
        </>
    }
}

fn make_button_click_callback(
    patient_id: i64,
    medication_id: i64,
    reminder_message_id: Option<i32>,
    fetch_callback: Callback<()>,
    time_taken: chrono::DateTime<chrono::Utc>,
    quantity: f64,
) -> Callback<MouseEvent> {
    Callback::from(move |e: MouseEvent| {
        e.prevent_default();
        let fetch_callback = fetch_callback.clone();
        wasm_bindgen_futures::spawn_local(async move {
            match api_log_dose(
                patient_id,
                medication_id,
                time_taken,
                quantity,
                reminder_message_id,
            )
            .await
            {
                Ok(_) => fetch_callback.emit(()),
                Err(e) => error!(format!("Failed to log dose: {}", e)),
            }
        })
    })
}

type ResponseState = UseStateHandle<Option<Result<PatientGetDosesResponse>>>;

fn make_fetch_callback(
    response: ResponseState,
    quantity: UseStateHandle<f64>,
    patient_id: i64,
    medication_id: i64,
) -> Callback<()> {
    Callback::from(move |_| {
        let patient_get_doses_response = response.clone();
        let quantity = quantity.clone();
        wasm_bindgen_futures::spawn_local(async move {
            patient_get_doses_response.set(None);
            let res = api_fetch(patient_id, medication_id).await;
            log_if_error("Failed to fetch medication info:", &res);
            if let Ok(r) = res.as_ref() {
                if let Some(q) = latest_quantity(r) {
                    quantity.set(q);
                }
            }
            patient_get_doses_response.set(Some(res));
        });
    })
}

fn latest_quantity(response: &PatientGetDosesResponse) -> Option<f64> {
    response
        .doses
        .iter()
        .map(|d| d.data.quantity)
        .find(|&d| d > 0.0) // "skipped" doses don't count
}
