use anyhow::{Result, bail};
use gloo_dialogs::confirm;
use gloo_net::http::Request;
use shared::api::{
    patient::Reminders,
    requests::{PatientMedicationCreateRequest, PatientMedicationUpdateRequest},
};
use web_sys::{HtmlInputElement, HtmlTextAreaElement};
use yew::prelude::*;

use crate::error_handling::log_if_error;

#[derive(PartialEq)]
pub enum MedicationEditMode {
    Create,
    Edit(i64, i64),
}

#[derive(Properties, PartialEq)]
pub struct MedicationEditProps {
    pub mode: MedicationEditMode,
    #[prop_or_default]
    pub name: String,
    #[prop_or_default]
    pub description: Option<String>,
    #[prop_or_default]
    pub reminders: Vec<String>,

    #[prop_or_default]
    pub onsave: Option<Callback<()>>,
    #[prop_or_default]
    pub ondelete: Option<Callback<()>>,
}

#[function_component(MedicationEdit)]
pub fn medication_edit(
    MedicationEditProps {
        mode,
        name,
        description,
        reminders,

        onsave,
        ondelete,
    }: &MedicationEditProps,
) -> Html {
    let name = use_state(|| name.clone());
    let description = use_state(|| description.clone());
    let reminders = use_state(|| reminders.join("\n"));

    let edit_name_callback = {
        let name = name.clone();
        Callback::from(move |ev: InputEvent| {
            let element: HtmlInputElement = ev.target_unchecked_into();
            name.set(element.value());
        })
    };

    let edit_description_callback = {
        let description = description.clone();
        Callback::from(move |ev: InputEvent| {
            let element: HtmlTextAreaElement = ev.target_unchecked_into();
            description.set(if element.value().is_empty() {
                None
            } else {
                Some(element.value())
            });
        })
    };

    let edit_reminders_callback = {
        let reminders = reminders.clone();
        Callback::from(move |ev: InputEvent| {
            let element: HtmlTextAreaElement = ev.target_unchecked_into();
            reminders.set(element.value());
        })
    };

    let delete_callback = make_delete_callback(mode, ondelete.clone());

    let save_callback = match mode {
        MedicationEditMode::Create => {
            make_create_callback(onsave.clone(), name.clone(), description.clone())
        }
        MedicationEditMode::Edit(patient_id, medication_id) => make_edit_callback(
            *patient_id,
            *medication_id,
            onsave.clone(),
            name.clone(),
            description.clone(),
            reminders.clone(),
        ),
    };

    render_form(
        mode,
        (*name).clone(),
        (*description).clone(),
        (*reminders).clone(),
        edit_name_callback,
        edit_description_callback,
        edit_reminders_callback,
        save_callback,
        delete_callback,
    )
}

#[allow(clippy::too_many_arguments)] // TODO
fn render_form(
    mode: &MedicationEditMode,
    name: String,
    description: Option<String>,
    reminders: String,
    edit_name_callback: Callback<InputEvent>,
    edit_description_callback: Callback<InputEvent>,
    edit_reminders_callback: Callback<InputEvent>,
    save_callback: Callback<MouseEvent>,
    delete_callback: Callback<MouseEvent>,
) -> Html {
    let schedule_explanation = explain_cron(&reminders.lines().collect::<Vec<_>>());
    let explanation = match &schedule_explanation {
        Ok(text) => html! { text.clone().join("; ") },
        Err(err) => html! {
            // Errors sometimes include location specifiers
             <pre>{err.to_string()}</pre>
        },
    };
    html! {
        <form>
            <input
                type="string"
                oninput={edit_name_callback}
                placeholder="Medication name"
                value={name}
            />
            <textarea
                oninput={edit_description_callback}
                placeholder="Medication description"
                value={description}
            />
            if let MedicationEditMode::Edit(_, _) = mode {
                <textarea
                    oninput={edit_reminders_callback}
                    aria-invalid={schedule_explanation.is_err().to_string()}
                    placeholder="Reminders (cron schedules)"
                    value={reminders}
                />
                <small>{explanation}</small>
            }
            <div class="grid">
                <button onclick={save_callback} disabled={schedule_explanation.is_err()}>
                    { match mode {
                    MedicationEditMode::Edit(_, _) => "Save",
                    MedicationEditMode::Create => "Create",
                } }
                </button>
                if let MedicationEditMode::Edit(_, _) = mode {
                    <button onclick={delete_callback} class="contrast">{ "Delete" }</button>
                }
            </div>
        </form>
    }
}

async fn api_create(req: &PatientMedicationCreateRequest) -> Result<()> {
    let api_url = "/api/medications";
    let res = Request::post(api_url)
        .json(&req)
        .expect("Failed to serialize medication data")
        .send()
        .await?;
    if !res.ok() {
        bail!(
            "Failed to create medication: {} ({})",
            res.status(),
            res.status_text()
        );
    }

    Ok(())
}

async fn api_save(
    patient_id: i64,
    medication_id: i64,
    req: &PatientMedicationUpdateRequest,
) -> Result<()> {
    let api_url = format!("/api/patients/{patient_id}/medications/{medication_id}");
    let res = Request::put(&api_url)
        .json(&req)
        .expect("Failed to serialize medication data")
        .send()
        .await?;
    if !res.ok() {
        bail!(
            "Failed to update medication: {} ({})",
            res.status(),
            res.status_text()
        );
    }

    Ok(())
}

async fn api_delete(medication_id: i64) -> Result<()> {
    let api_url = format!("/api/medications/{medication_id}");
    let res = Request::delete(&api_url).send().await?;
    if !res.ok() {
        bail!(
            "Failed to delete medication: {} ({})",
            res.status(),
            res.status_text()
        );
    }

    Ok(())
}

fn make_delete_callback(
    mode: &MedicationEditMode,
    ondelete: Option<Callback<()>>,
) -> Callback<MouseEvent> {
    let MedicationEditMode::Edit(_, medication_id) = mode else {
        return Callback::from(|_| ());
    };

    let medication_id = *medication_id;

    Callback::from(move |ev: MouseEvent| {
        let ondelete = ondelete.clone();
        ev.prevent_default();
        if !confirm("Are you sure you want to delete this medication?") {
            return;
        }
        wasm_bindgen_futures::spawn_local(async move {
            let res = api_delete(medication_id).await;
            log_if_error("Failed to delete medication: ", &res);
            if res.is_ok() {
                if let Some(ondelete) = ondelete {
                    ondelete.emit(())
                }
            }
        })
    })
}

fn make_create_callback(
    onsave: Option<Callback<()>>,
    name: UseStateHandle<String>,
    description: UseStateHandle<Option<String>>,
) -> Callback<MouseEvent> {
    Callback::from(move |ev: MouseEvent| {
        ev.prevent_default();
        let name = name.clone();
        let description = description.clone();
        let onsave = onsave.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let req = PatientMedicationCreateRequest {
                name: (*name).clone(),
                description: (*description).clone(),
            };
            let res = api_create(&req).await;
            log_if_error("Failed to create medication: ", &res);
            if res.is_ok() {
                if let Some(onsave) = onsave {
                    onsave.emit(());
                }
            }
        })
    })
}

fn make_edit_callback(
    patient_id: i64,
    medication_id: i64,
    onsave: Option<Callback<()>>,
    name: UseStateHandle<String>,
    description: UseStateHandle<Option<String>>,
    reminders: UseStateHandle<String>,
) -> Callback<MouseEvent> {
    Callback::from(move |ev: MouseEvent| {
        ev.prevent_default();
        let name = name.clone();
        let description = description.clone();
        let reminders = reminders.clone();
        let onsave = onsave.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let req = PatientMedicationUpdateRequest {
                medication: PatientMedicationCreateRequest {
                    name: (*name).clone(),
                    description: (*description).clone(),
                },
                reminders: Reminders {
                    cron_schedules: (*reminders).lines().map(String::from).collect(),
                },
            };
            let res = api_save(patient_id, medication_id, &req).await;
            log_if_error("Failed to update medication: ", &res);
            if res.is_ok() {
                if let Some(onsave) = onsave {
                    onsave.emit(());
                }
            }
        })
    })
}

fn explain_cron(schedules: &[&str]) -> Result<Vec<String>> {
    let result = schedules
        .iter()
        .map(|sched| {
            if sched.split(' ').count() != 6 {
                bail!("Only 6-part cron schedules are supported: {sched:?}");
            }

            // Check that the cron schedule looks valid; this is because the
            // cron_descriptor package panics on some invalid schedules.
            <cron::Schedule as std::str::FromStr>::from_str(sched)?;

            cron_descriptor::cronparser::cron_expression_descriptor::get_description_cron(sched)
                .map_err(|e| anyhow::anyhow!("{e:?}"))
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(result)
}
