use anyhow::{Result, bail};
use gloo_console::error;
use shared::api::{patient, requests::PatientCreateRequest};
use web_sys::HtmlInputElement;
use yew::prelude::*;

use crate::{
    components::patient_list::PatientList,
    error_handling::{error_waiting_or, log_if_error},
    username,
};
use gloo_net::http::Request;

async fn fetch() -> Result<Vec<patient::Patient>> {
    let res = Request::get("/api/patients").send().await?;
    if !res.ok() {
        bail!(
            "Fetching patient list returned non-OK response: {} {}",
            res.status(),
            res.status_text()
        );
    }
    Ok(res.json().await?)
}

#[function_component(Home)]
pub fn home() -> Html {
    let patients = use_state(|| None);

    {
        let patients = patients.clone();
        use_effect_with((), move |_| {
            let patients = patients.clone();
            wasm_bindgen_futures::spawn_local(async move {
                patients.set(None);
                let res = fetch().await;
                log_if_error("Failed to fetch patient list:", &res);
                patients.set(Some(res));
            });
        });
    }

    let patient_list = error_waiting_or(patients.as_ref(), move |patients| {
        html! {
            <>
                <h1>{ "Select Patient" }</h1>
                <PatientList patients={(*patients).clone()} />
            </>
        }
    });

    let on_username_change = {
        Callback::from(move |e: yew::Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            username::set(input.value());
        })
    };

    let current_username = username::get();
    let user_name_picker = html! {
        <label for="username">
            { "User name:" }
            <input
                type="text"
                id="username"
                placeholder="Who's giving the medication?"
                onchange={on_username_change.clone()}
                value={current_username.clone()}
            />
        </label>
    };

    let create_patient_callback = {
        let patients = patients.clone();
        Callback::from(move |req: PatientCreateRequest| {
            let patients = patients.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let res = Request::post("/api/patients")
                    .json(&req)
                    .unwrap()
                    .send()
                    .await;
                if let Ok(response) = res {
                    if response.ok() {
                        // Refresh patient list
                        patients.set(None);
                        let res = fetch().await;
                        log_if_error("Failed to fetch patient list:", &res);
                        patients.set(Some(res));
                    }
                    // TODO: !response.ok() is not explicitly handled; you've
                    // done this error handling better elsewhere in the
                    // codebase.
                } else if let Err(err) = res {
                    error!(format!("Failed to create user: {err}"));
                }
            });
        })
    };

    let create_patient = html! {
        <details>
            <summary>{ "Create patient" }</summary>
            <PatientSettings
                name=""
                telegram_group_id={None}
                onsave={create_patient_callback}
            />
        </details>
    };

    html! {
        <>
            { patient_list }
            <hr />
            { user_name_picker }
            <hr />
            { create_patient }
        </>
    }
}

#[derive(Properties, PartialEq)]
pub struct PatientSettingsProps {
    pub name: String,
    pub telegram_group_id: Option<i64>,

    pub onsave: Callback<PatientCreateRequest>,
}

#[function_component(PatientSettings)]
pub fn patient_settings(
    PatientSettingsProps {
        name,
        telegram_group_id,
        onsave,
    }: &PatientSettingsProps,
) -> Html {
    // TODO: Move this in with the rest of the patient stuff

    let name = use_state(|| name.clone());
    let telegram_group_id = use_state(|| *telegram_group_id);

    let onclick = {
        let name = name.clone();
        let telegram_group_id = telegram_group_id.clone();
        let onsave = onsave.clone();

        Callback::from(move |_: MouseEvent| {
            let name = (*name).clone();
            let telegram_group_id = *telegram_group_id;

            onsave.emit(PatientCreateRequest {
                name,
                telegram_group_id,
            });
        })
    };

    let on_name_change = {
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let value = input.value();
            name.set(value);
        })
    };

    let on_telegram_group_id_change = {
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let value = input.value();
            telegram_group_id.set(if value.is_empty() {
                None
            } else {
                value.parse().ok()
            });
        })
    };

    html! {
        <fieldset role="group">
            <input type="text" placeholder="Name" aria-label="Name" oninput={on_name_change} />
            <input
                type="number"
                placeholder="Telegram Group ID"
                aria-label="Telegram Group ID"
                oninput={on_telegram_group_id_change}
            />
            <button onclick={onclick}>{ "Save" }</button>
        </fieldset>
    }
}
