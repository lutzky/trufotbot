use anyhow::{Result, bail};
use gloo_net::http::Request;
use shared::api::{
    dose::CreateDose,
    responses::{self, GetDoseResponse},
};
use yew::prelude::*;
use yew_router::prelude::*;

use crate::{
    components::dose::Dose,
    error_handling::{self, log_if_error},
    routes::Route,
};

async fn api_fetch(
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

async fn api_save(
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

async fn api_delete(patient_id: i64, medication_id: i64, dose_id: i64) -> Result<()> {
    let api_url = format!("/api/patients/{patient_id}/medications/{medication_id}/doses/{dose_id}");
    let res = Request::delete(&api_url).send().await?;
    if !res.ok() {
        bail!(
            "Failed to delete medication dose: {} {}",
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

type ResponseState = UseStateHandle<Option<Result<GetDoseResponse>>>;

fn make_fetch_callback(
    response: ResponseState,
    patient_id: i64,
    medication_id: i64,
    dose_id: i64,
) -> Callback<()> {
    Callback::from(move |_: ()| {
        let response = response.clone();

        wasm_bindgen_futures::spawn_local(async move {
            response.set(None);
            let res = api_fetch(patient_id, medication_id, dose_id).await;
            log_if_error("Failed to fetch medication info:", &res);
            response.set(Some(res));
        });
    })
}

fn make_save_callback(
    response: ResponseState,
    save_button_state: UseStateHandle<ButtonState>,
    patient_id: i64,
    medication_id: i64,
    dose_id: i64,
) -> Callback<MouseEvent> {
    Callback::from(move |e: MouseEvent| {
        e.prevent_default();
        let Some(Ok(current_response)) = &(*response) else {
            return;
        };
        let save_button_state = save_button_state.clone();
        let dose_data = current_response.dose.data.clone();
        wasm_bindgen_futures::spawn_local(async move {
            save_button_state.set(ButtonState::Loading);
            let res = api_save(patient_id, medication_id, dose_id, &dose_data).await;
            log_if_error("Failed to save medication dose:", &res);
            save_button_state.set(res.into());
        });
    })
}

fn make_delete_callback(
    navigator: Navigator,
    patient_id: i64,
    medication_id: i64,
    dose_id: i64,
) -> Callback<MouseEvent> {
    Callback::from(move |e: MouseEvent| {
        let navigator = navigator.clone();

        e.prevent_default();

        wasm_bindgen_futures::spawn_local(async move {
            let confirmed = gloo_dialogs::confirm("Are you sure you want to delete this dose?");
            if !confirmed {
                return;
            }
            let res = api_delete(patient_id, medication_id, dose_id).await;
            log_if_error("Failed to delete dose: ", &res);
            if res.is_ok() {
                navigator.push(&Route::PatientMedicationDetail {
                    patient_id,
                    medication_id,
                });
            }
        });
    })
}

fn make_update_callback(
    response: ResponseState,
    save_button_state: UseStateHandle<ButtonState>,
) -> Callback<CreateDose> {
    Callback::from(move |data| {
        let Some(Ok(current_response)) = &(*response) else {
            return;
        };
        let mut current_response = current_response.clone();
        current_response.dose.data = data;
        response.set(Some(Ok(current_response)));
        save_button_state.set(ButtonState::Ready);
    })
}

fn render_content(
    response: &GetDoseResponse,
    update_callback: Callback<CreateDose>,
    save_callback: Callback<MouseEvent>,
    delete_callback: Callback<MouseEvent>,
    save_button_state: &ButtonState,
) -> Html {
    use ButtonState as BS;

    let save_busy = matches!(save_button_state, BS::Loading);
    let save_disabled = !matches!(save_button_state, BS::Ready);
    let save_class = match save_button_state {
        BS::Err(_) => "pico-background-red",
        _ => "",
    };
    let save_label = match save_button_state {
        BS::Ready => "Save",
        BS::Loading => "Saving...",
        BS::Ok => "Saved",
        BS::Err(s) => s,
    };

    html! {
        <>
            <hgroup>
                <h1>{ format!("Dose {}", response.dose.id) }</h1>
                <p>{ format!("{} for {}", response.medication_name, response.patient_name) }</p>
            </hgroup>
            <form>
                <Dose data={response.clone().dose.data} oninput={update_callback} />
                <div class="grid">
                    <button
                        aria-busy={save_busy.to_string()}
                        disabled={save_disabled}
                        class={save_class}
                        onclick={save_callback}
                    >
                        { save_label }
                    </button>
                    <button class="contrast" onclick={delete_callback}>{ "Delete" }</button>
                </div>
            </form>
        </>
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

    let navigator = use_navigator().expect("Navigator not available");

    let delete_callback =
        make_delete_callback(navigator.clone(), *patient_id, *medication_id, *dose_id);
    let fetch_callback =
        make_fetch_callback(response.clone(), *patient_id, *medication_id, *dose_id);
    let save_callback = make_save_callback(
        response.clone(),
        save_button_state.clone(),
        *patient_id,
        *medication_id,
        *dose_id,
    );
    let update_callback = make_update_callback(response.clone(), save_button_state.clone());

    use_effect_with((), move |_| {
        fetch_callback.emit(());
    });

    let content = error_handling::error_waiting_or(response.as_ref(), |response| {
        render_content(
            response,
            update_callback.clone(),
            save_callback.clone(),
            delete_callback.clone(),
            &save_button_state,
        )
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
