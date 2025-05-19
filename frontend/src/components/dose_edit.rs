use anyhow::{Result, bail};
use gloo_console::{error, info};
use gloo_net::http::Request;
use shared::api::{
    dose::CreateDose,
    responses::{self},
};
use yew::prelude::*;
use yew_router::prelude::*;

use crate::{
    components::dose::Dose,
    error_handling::{self, log_if_error},
    routes::Route,
};

async fn fetch(
    patient_id: i64,
    medication_id: i64,
    dose_id: i64,
) -> Result<responses::GetDoseResponse> {
    let api_url = format!("/api/patients/{patient_id}/medications/{medication_id}/doses/{dose_id}");
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

async fn save(
    patient_id: i64,
    medication_id: i64,
    dose_id: i64,
    payload: &CreateDose,
) -> Result<()> {
    let api_url = format!("/api/patients/{patient_id}/medications/{medication_id}/doses/{dose_id}");
    let res = Request::put(&api_url).json(payload)?.send().await?;
    if !res.ok() {
        bail!(
            "Failed to update medication dose: {} {}",
            res.status(),
            res.status_text()
        );
    }
    Ok(())
}

#[derive(Properties, PartialEq)]
pub struct DoseEditProps {
    pub patient_id: i64,
    pub medication_id: i64,
    pub dose_id: i64,
}

enum ButtonState {
    Ready,
    Loading,
    Ok,
    Err(String),
}

impl<T> From<Result<T>> for ButtonState {
    fn from(value: Result<T>) -> Self {
        use ButtonState as BS;
        match value {
            Ok(_) => BS::Ok,
            Err(e) => BS::Err(e.to_string()),
        }
    }
}

impl<T> From<Option<Result<T>>> for ButtonState {
    fn from(value: Option<Result<T>>) -> Self {
        use ButtonState as BS;
        match value {
            Some(r) => r.into(),
            None => BS::Loading,
        }
    }
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
    let save_button_state = use_state(|| ButtonState::Ready);

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

    let save_callback = {
        let patient_id = *patient_id;
        let medication_id = *medication_id;
        let dose_id = *dose_id;
        let response = response.clone();
        let save_button_state = save_button_state.clone();

        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let Some(Ok(current_response)) = &(*response) else {
                return;
            };
            let save_button_state = save_button_state.clone();
            let dose_data = current_response.dose.data.clone();
            wasm_bindgen_futures::spawn_local(async move {
                save_button_state.set(ButtonState::Loading);
                let res = save(patient_id, medication_id, dose_id, &dose_data).await;
                log_if_error("Failed to save medication dose:", &res);
                save_button_state.set(res.into());
            });
        })
    };

    let navigator = use_navigator().expect("Navigator not available");

    let delete_callback = {
        let patient_id = *patient_id;
        let medication_id = *medication_id;
        let dose_id = *dose_id;

        Callback::from(move |e: MouseEvent| {
            let navigator = navigator.clone();

            e.prevent_default();

            let api_url =
                format!("/api/patients/{patient_id}/medications/{medication_id}/doses/{dose_id}");

            wasm_bindgen_futures::spawn_local(async move {
                let confirmed = gloo_dialogs::confirm("Are you sure you want to delete this dose?");
                if !confirmed {
                    return;
                }
                let res = Request::delete(&api_url).send().await;

                match res {
                    Ok(response) if response.ok() => {
                        info!("Dose deleted successfully");
                        navigator.push(&Route::PatientMedicationDetail {
                            patient_id,
                            medication_id,
                        });
                    }
                    Ok(response) => {
                        error!(
                            "Failed to delete dose:",
                            response.status(),
                            response.status_text()
                        );
                    }
                    Err(err) => {
                        error!(format!("Error occurred while deleting dose: {err:?}"));
                    }
                }
            });
        })
    };

    use_effect_with((), move |_| {
        fetch_callback.emit(());
    });

    let update_callback = {
        let response = response.clone();
        let save_button_state = save_button_state.clone();
        Callback::from(move |data| {
            let Some(Ok(current_response)) = &(*response) else {
                return;
            };
            let mut current_response = current_response.clone();
            current_response.dose.data = data;
            response.set(Some(Ok(current_response)));
            save_button_state.set(ButtonState::Ready);
        })
    };

    let content = error_handling::error_waiting_or(response.as_ref(), |response| {
        use ButtonState as BS;
        let update_dose_callback = update_callback.clone();
        let save_callback = save_callback.clone();
        let delete_callback = delete_callback.clone();

        html! {
            <>
                <hgroup>
                    <h1>{ format!("Dose {dose_id}") }</h1>
                    <p>{ format!("{} for {}", response.medication_name, response.patient_name) }</p>
                </hgroup>
                <form>
                    <Dose data={response.clone().dose.data} oninput={update_dose_callback} />
                    <div class="grid">
                        <button
                            aria-busy={match *save_button_state {
                                BS::Loading => "true",
                                _ => "false",
                            }}
                            disabled={!matches!(*save_button_state, BS::Ready)}
                            class={match *save_button_state {
                                BS::Err(_) => "pico-background-red",
                                _ => "",
                            }}
                            onclick={save_callback}
                        >
                            { match &*save_button_state {
                                BS::Ready => "Save",
                                BS::Loading => "Saving...",
                                BS::Ok => "Saved",
                                BS::Err(s) => s,
                            } }
                        </button>
                        <button class="contrast" onclick={delete_callback}>{ "Delete" }</button>
                    </div>
                </form>
            </>
        }
    });

    let back_route = Route::PatientMedicationDetail {
        patient_id: *patient_id,
        medication_id: *medication_id,
    };

    html! {
        <>
            <Link<Route> classes="secondary" to={back_route}>
                { "< Back to medication details" }
            </Link<Route>>
            { content }
        </>
    }
}
