use anyhow::{Result, bail};
use shared::api::{patient, requests::PatientCreateRequest};
use web_sys::HtmlInputElement;
use yew::prelude::*;

use crate::{
    components::{patient_list::PatientList, patient_settings::PatientSettings},
    error_handling::{error_waiting_or, log_if_error},
    username,
};
use gloo_net::http::Request;

async fn api_fetch() -> Result<Vec<patient::Patient>> {
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

async fn api_create_patient(req: PatientCreateRequest) -> Result<()> {
    let res = Request::post("/api/patients").json(&req)?.send().await?;
    if !res.ok() {
        bail!(
            "Creating patient returned non-OK response: {} {}",
            res.status(),
            res.status_text()
        );
    }
    Ok(())
}

type ResponseState = UseStateHandle<Option<Result<Vec<patient::Patient>>>>;

fn make_fetch_callback(patients: ResponseState) -> Callback<()> {
    Callback::from(move |_| {
        let patients = patients.clone();
        wasm_bindgen_futures::spawn_local(async move {
            patients.set(None);
            let res = api_fetch().await;
            log_if_error("Failed to fetch patient list:", &res);
            patients.set(Some(res));
        });
    })
}

fn make_create_patient_callback(fetch_callback: Callback<()>) -> Callback<PatientCreateRequest> {
    Callback::from(move |req: PatientCreateRequest| {
        let fetch_callback = fetch_callback.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let res = api_create_patient(req).await;
            log_if_error("Failed to create patient", &res);
            if res.is_ok() {
                fetch_callback.emit(());
            }
        });
    })
}

#[function_component(Home)]
pub fn home() -> Html {
    let response: UseStateHandle<Option<Result<Vec<patient::Patient>>>> = use_state(|| None);

    let fetch_callback = make_fetch_callback(response.clone());

    {
        let fetch_callback = fetch_callback.clone();
        use_effect_with((), move |_| {
            fetch_callback.emit(());
        });
    }

    let patient_list = error_waiting_or(response.as_ref(), move |patients| {
        html! { <PatientList patients={(*patients).clone()} /> }
    });

    html! {
        <>
            <h1>{ "Select Patient" }</h1>
            { patient_list }
            <hr />
            { render_user_name_picker() }
            <hr />
            { render_create_patient(fetch_callback.clone()) }
        </>
    }
}

fn render_create_patient(fetch_callback: Callback<()>) -> Html {
    let create_patient_callback = make_create_patient_callback(fetch_callback.clone());
    html! {
        <details>
            <summary>{ "Create patient" }</summary>
            <PatientSettings
                group=true
                name=""
                telegram_group_id={None}
                onsave={create_patient_callback}
            />
        </details>
    }
}

fn render_user_name_picker() -> Html {
    let current_username = username::get();

    let on_username_change = {
        Callback::from(move |e: yew::Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            username::set(input.value());
        })
    };

    html! {
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
    }
}
