use shared::api::dose::CreateDose;
use web_sys::HtmlInputElement;
use yew::prelude::*;

use crate::time::LocalTime;

#[derive(Properties, PartialEq)]
pub struct DoseProps {
    pub data: CreateDose,
    pub oninput: Callback<CreateDose>,

    #[prop_or(true)]
    pub show_noted_by: bool,
}

#[function_component(Dose)]
pub fn dose_component(
    DoseProps {
        data: initial_data,
        oninput,
        show_noted_by,
    }: &DoseProps,
) -> Html {
    let data = use_state(|| initial_data.clone());

    let update_data = {
        let data = data.clone();
        let oninput = oninput.clone();
        move |update_fn: Box<dyn FnOnce(&mut CreateDose)>| {
            let data = data.clone();
            let mut updated_data = (*data).clone();
            update_fn(&mut updated_data);
            data.set(updated_data.clone());
            oninput.emit(updated_data);
        }
    };

    let set_time = {
        let update_data = update_data.clone();
        Callback::from(move |t: chrono::DateTime<chrono::Utc>| {
            update_data(Box::new(move |dose_data| {
                dose_data.taken_at = t;
            }));
        })
    };

    let set_noted_by = {
        let update_data = update_data.clone();
        Callback::from(move |e: InputEvent| {
            let noted_by_user = match e.target_unchecked_into::<HtmlInputElement>().value() {
                s if s.is_empty() => None,
                s => Some(s),
            };
            update_data(Box::new(move |dose_data| {
                dose_data.noted_by_user = noted_by_user;
            }));
        })
    };

    let set_quantity = {
        let update_data = update_data.clone();
        Callback::from(move |e: InputEvent| {
            let quantity = e
                .target_unchecked_into::<HtmlInputElement>()
                .value()
                .parse()
                .unwrap_or(0.0);
            update_data(Box::new(move |dose_data| {
                dose_data.quantity = quantity;
            }));
        })
    };

    let data = (*data).clone();

    html! {
        <>
            <LocalTime onchange={set_time} utc_time={data.taken_at} />
            <input
                name="quantity"
                oninput={set_quantity}
                aria-label="Quantity"
                type="number"
                placeholder="How much of it?"
                value={format!("{}",data.quantity)}
            />
            { if *show_noted_by {
                html! {
                    <input
                        name="noted-by"
                        oninput={set_noted_by}
                        aria-label="Noted by"
                        placeholder="Who gave this?"
                        type="text"
                        value={data.noted_by_user}
                    />
                }
            } else{ html! {}} }
        </>
    }
}
