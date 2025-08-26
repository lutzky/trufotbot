use anyhow::{Result, bail};
use gloo_dialogs::confirm;
use gloo_net::http::Request;
use shared::api::{
    medication::DoseLimit,
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
    pub dose_limits: Vec<DoseLimit>,
    #[prop_or_default]
    pub inventory: Option<f64>,

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
        dose_limits,
        inventory,

        onsave,
        ondelete,
    }: &MedicationEditProps,
) -> Html {
    let name = use_state(|| name.clone());
    let description = use_state(|| description.clone());
    let reminders = use_state(|| reminders.join("\n"));
    let dose_limits = use_state(|| DoseLimit::string_from_vec(dose_limits));

    let inventory: UseStateHandle<String> =
        use_state(|| inventory.map(|x: f64| x.to_string()).unwrap_or_default());

    let callbacks = FormCallbacks {
        edit_name: {
            let name = name.clone();
            Callback::from(move |ev: InputEvent| {
                let element: HtmlInputElement = ev.target_unchecked_into();
                name.set(element.value());
            })
        },

        edit_description: {
            let description = description.clone();
            Callback::from(move |ev: InputEvent| {
                let element: HtmlTextAreaElement = ev.target_unchecked_into();
                description.set(if element.value().is_empty() {
                    None
                } else {
                    Some(element.value())
                });
            })
        },

        edit_reminders: {
            let reminders = reminders.clone();
            Callback::from(move |ev: InputEvent| {
                let element: HtmlTextAreaElement = ev.target_unchecked_into();
                reminders.set(element.value());
            })
        },

        edit_dose_limits: {
            let dose_limits = dose_limits.clone();
            Callback::from(move |ev: InputEvent| {
                let element: HtmlTextAreaElement = ev.target_unchecked_into();
                dose_limits.set(element.value());
            })
        },

        edit_inventory: {
            let inventory = inventory.clone();
            Callback::from(move |ev: InputEvent| {
                let element: HtmlInputElement = ev.target_unchecked_into();
                inventory.set(element.value());
            })
        },

        delete: make_delete_callback(mode, ondelete.clone()),

        save: match mode {
            MedicationEditMode::Create => {
                make_create_callback(onsave.clone(), name.clone(), description.clone())
            }
            MedicationEditMode::Edit(patient_id, medication_id) => make_edit_callback(
                *patient_id,
                *medication_id,
                onsave.clone(),
                EditCallbackStateHandles {
                    name: name.clone(),
                    description: description.clone(),
                    reminders: reminders.clone(),
                    dose_limits: dose_limits.clone(),
                    inventory: inventory.clone(),
                },
            ),
        },
    };

    render_form(
        mode,
        (*name).clone(),
        (*description).clone(),
        (*reminders).clone(),
        (*dose_limits).clone(),
        (*inventory).clone(),
        callbacks,
    )
}

struct FormCallbacks {
    edit_name: Callback<InputEvent>,
    edit_description: Callback<InputEvent>,
    edit_reminders: Callback<InputEvent>,
    edit_dose_limits: Callback<InputEvent>,
    edit_inventory: Callback<InputEvent>,
    save: Callback<MouseEvent>,
    delete: Callback<MouseEvent>,
}

fn render_form(
    mode: &MedicationEditMode,
    name: String,
    description: Option<String>,
    reminders: String,
    dose_limits: String,
    inventory: String,
    callbacks: FormCallbacks,
) -> Html {
    let schedule_explanations = explain_cron(&reminders.lines().collect::<Vec<_>>());
    let schedule_explanations_text = match &schedule_explanations {
        Ok(text) => html! { text.clone().join("; ") },
        Err(err) => html! {
            // Errors sometimes include location specifiers
            <pre>{ err.to_string() }</pre>
        },
    };
    let dose_limits_check = DoseLimit::vec_from_string(&dose_limits);
    let enable_save = schedule_explanations.is_ok() && dose_limits_check.is_ok();
    html! {
        <form>
            <input
                type="string"
                oninput={callbacks.edit_name}
                placeholder="Medication name"
                value={name}
            />
            <textarea
                oninput={callbacks.edit_description}
                placeholder="Medication description"
                value={description}
            />
            if let MedicationEditMode::Edit(_, _) = mode {
                <label>
                    { "Inventory" }
                    <input
                        type="number"
                        oninput={callbacks.edit_inventory}
                        placeholder="Inventory"
                        value={inventory}
                    />
                </label>
                <textarea
                    oninput={callbacks.edit_reminders}
                    aria-invalid={schedule_explanations.is_err().to_string()}
                    placeholder="Reminders (cron schedules)"
                    value={reminders}
                />
                <small>{ schedule_explanations_text }</small>
                <textarea
                    oninput={callbacks.edit_dose_limits}
                    aria-invalid={dose_limits_check.is_err().to_string()}
                    placeholder="Limits (hours:amount,hours:amount,...)"
                    value={dose_limits}
                />
            }
            <div class="grid">
                <button onclick={callbacks.save} disabled={!enable_save}>
                    { match mode {
                    MedicationEditMode::Edit(_, _) => "Save",
                    MedicationEditMode::Create => "Create",
                } }
                </button>
                if let MedicationEditMode::Edit(_, _) = mode {
                    <button onclick={callbacks.delete} class="contrast">{ "Delete" }</button>
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
            if res.is_ok()
                && let Some(ondelete) = ondelete
            {
                ondelete.emit(())
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
                inventory: None,
                dose_limits: vec![],
            };
            let res = api_create(&req).await;
            log_if_error("Failed to create medication: ", &res);
            if res.is_ok()
                && let Some(onsave) = onsave
            {
                onsave.emit(());
            }
        })
    })
}

struct EditCallbackStateHandles {
    name: UseStateHandle<String>,
    description: UseStateHandle<Option<String>>,
    reminders: UseStateHandle<String>,
    dose_limits: UseStateHandle<String>,
    inventory: UseStateHandle<String>,
}

fn make_edit_callback(
    patient_id: i64,
    medication_id: i64,
    onsave: Option<Callback<()>>,
    state: EditCallbackStateHandles,
) -> Callback<MouseEvent> {
    Callback::from(move |ev: MouseEvent| {
        ev.prevent_default();
        let name = state.name.clone();
        let description = state.description.clone();
        let reminders = state.reminders.clone();
        let onsave = onsave.clone();
        let dose_limits = state.dose_limits.clone();
        let inventory = match (*state.inventory).clone() {
            s if s.is_empty() => None,
            s => {
                let res = s.parse::<f64>();
                if let Err(res) = &res {
                    gloo_console::error!(format!("Failed to parse inventory: {res:?}"));
                }
                res.ok()
            }
        };
        wasm_bindgen_futures::spawn_local(async move {
            let Ok(dose_limits) = DoseLimit::vec_from_string(&dose_limits) else {
                gloo_console::error!("Invalid dose limits: ", (*dose_limits).clone());
                return;
            };

            let req = PatientMedicationUpdateRequest {
                medication: PatientMedicationCreateRequest {
                    name: (*name).clone(),
                    description: (*description).clone(),
                    inventory,
                    dose_limits,
                },
                reminders: Reminders {
                    cron_schedules: (*reminders).lines().map(String::from).collect(),
                },
            };
            let res = api_save(patient_id, medication_id, &req).await;
            log_if_error("Failed to update medication: ", &res);
            if res.is_ok()
                && let Some(onsave) = onsave
            {
                onsave.emit(());
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
