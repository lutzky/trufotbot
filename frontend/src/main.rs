use gloo_console::info;
use gloo_net::http::Request;
use yew::prelude::*;

mod model {
    // TODO: Merge with the backend
    #[derive(PartialEq, Clone, serde::Deserialize)]
    pub struct Patient {
        pub id: i64,
        pub name: String,
    }
}

#[derive(Properties, PartialEq)]
struct PatientListProps {
    patients: Vec<model::Patient>,
}

#[function_component(PatientList)]
fn patient_list(PatientListProps { patients }: &PatientListProps) -> Html {
    patients
        .iter()
        .map(|patient| {
            let ping_them = {
                let patient = patient.clone();
                Callback::from(move |_: MouseEvent| {
                    wasm_bindgen_futures::spawn_local(async move {
                        info!("Sending ping for patient", patient.id);
                        Request::post(&format!("/api/patients/{}/ping", patient.id))
                            .send()
                            .await
                            .unwrap();
                    })
                })
            };
            html! {
            <div class="patient" style="border: 1px solid black; padding: 10px;">
                <h1>{ &patient.name }</h1>
                <p>{ "Patient details go here." }</p>
                <button onclick={ping_them}>{ "Ping" }</button>
            </div>}
        })
        .collect()
}

#[function_component]
fn App() -> Html {
    let patients = use_state(|| vec![]);
    {
        let patients = patients.clone();
        use_effect_with((), move |_| {
            let patients = patients.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let fetched_patients: Vec<model::Patient> = Request::get("/api/patients")
                    .send()
                    .await
                    .unwrap()
                    .json()
                    .await
                    .unwrap();
                patients.set(fetched_patients);
            });
            || ()
        });
    }

    html! {
        <PatientList patients={(*patients).clone()} />
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
