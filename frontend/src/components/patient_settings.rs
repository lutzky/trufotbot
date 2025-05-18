use shared::api::requests::PatientCreateRequest;
use web_sys::HtmlInputElement;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct PatientSettingsProps {
    pub name: String,
    pub telegram_group_id: Option<i64>,
    pub group: bool,

    pub onsave: Callback<PatientCreateRequest>,
}

#[function_component(PatientSettings)]
pub fn patient_settings(
    PatientSettingsProps {
        name,
        telegram_group_id,
        group,
        onsave,
    }: &PatientSettingsProps,
) -> Html {
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
        let name = name.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let value = input.value();
            name.set(value);
        })
    };

    let on_telegram_group_id_change = {
        let telegram_group_id = telegram_group_id.clone();
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

    let display_telegram_group_id = match *telegram_group_id {
        None => "".to_string(),
        Some(id) => format!("{id}"),
    };

    html! {
        <>
            <fieldset role="group">
                <input
                    type="text"
                    placeholder="Name"
                    value={(*name).clone()}
                    aria-label="Name"
                    oninput={on_name_change}
                />
                <input
                    type="number"
                    placeholder="Telegram Group ID"
                    aria-label="Telegram Group ID"
                    value={display_telegram_group_id}
                    oninput={on_telegram_group_id_change}
                />
                if *group {
                    <button onclick={onclick.clone()}>{ "Save" }</button>
                }
            </fieldset>
            if !*group {
                <div class="grid">
                    <button onclick={onclick.clone()}>{ "Save" }</button>
                </div>
            }
        </>
    }
}
