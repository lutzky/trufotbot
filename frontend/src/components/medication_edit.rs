use anyhow::{Result, bail};
use gloo_dialogs::confirm;
use gloo_net::http::Request;
use shared::api::requests::PatientMedicationUpdateRequest;
use web_sys::HtmlInputElement;
use yew::prelude::*;

use crate::error_handling::log_if_error;

#[derive(Properties, PartialEq)]
pub struct MedicationEditProps {
    pub patient_id: i64,
    pub medication_id: i64,
    pub name: String,
    pub description: Option<String>,

    pub onsave: Callback<()>,
    pub ondelete: Callback<()>,
}

// TODO: Somewhere create a button for adding new medication using this form

// TODO: Simplification idea - we can have MedicationEdit render a form with or
// without the delete button based on whether or not it gets an ID (make it an
// Option). It can own its handler calls and everything. Critically, the same
// approach can apply to doses and patients!

#[function_component(MedicationEdit)]
pub fn medication_edit(
    MedicationEditProps {
        patient_id,
        medication_id,
        name,
        description,

        onsave,
        ondelete,
    }: &MedicationEditProps,
) -> Html {
    let name = use_state(|| name.clone());
    let description = use_state(|| description.clone());

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
            let element: HtmlInputElement = ev.target_unchecked_into();
            description.set(if element.value().is_empty() {
                None
            } else {
                Some(element.value())
            });
        })
    };

    let delete_callback = {
        let ondelete = ondelete.clone();
        let medication_id = *medication_id;
        Callback::from(move |ev: MouseEvent| {
            let ondelete = ondelete.clone();
            ev.prevent_default();
            if !confirm("Are you sure you want to delete this medication?") {
                return;
            }
            wasm_bindgen_futures::spawn_local(async move {
                let res = delete(medication_id).await;
                log_if_error("Failed to delete medication: ", &res);
                if res.is_ok() {
                    ondelete.emit(())
                }
            })
        })
    };

    let save_callback = {
        let onsave = onsave.clone();
        let name = name.clone();
        let description = description.clone();
        let patient_id = *patient_id;
        let medication_id = *medication_id;
        Callback::from(move |ev: MouseEvent| {
            let onsave = onsave.clone();
            ev.prevent_default();
            let name = name.clone();
            let description = description.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let req = PatientMedicationUpdateRequest {
                    name: (*name).clone(),
                    description: (*description).clone(),
                };

                let res = save(patient_id, medication_id, &req).await;
                log_if_error("Failed to update medication: ", &res);
                if res.is_ok() {
                    onsave.emit(());
                }
            })
        })
    };

    html! {
        <form>
            <input type="string" oninput={edit_name_callback} placeholder="Medication name" value={(*name).clone()} />
            <textarea oninput={edit_description_callback} placeholder="Medication description" value={(*description).clone()}/>
            <div class="grid">
                <button onclick={save_callback}>{ "Save" }</button>
                <button onclick={delete_callback} class="contrast">{ "Delete" }</button>
            </div>
        </form>
    }
}

async fn save(
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

async fn delete(medication_id: i64) -> Result<()> {
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
