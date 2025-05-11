use anyhow::{Result, bail};
use shared::api::patient;
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
        html! { <PatientList patients={(*patients).clone()} /> }
    });

    let on_username_change = {
        Callback::from(move |e: yew::Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            username::set(input.value());
        })
    };

    let current_username = username::get();

    html! {
        <>
            <h1>{ "Select Patient" }</h1>
            { patient_list }
            <hr />
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
        </>
    }
}
