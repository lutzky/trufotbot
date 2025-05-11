use anyhow::Result;
use gloo_console::error;
use yew::html;

pub fn log_if_error<T>(prefix: &str, r: &Result<T>) {
    let Err(e) = r else {
        return;
    };

    error!(prefix, e.to_string());
}

pub fn error_waiting_or<T>(resp: Option<&Result<T>>, f: impl Fn(&T) -> yew::Html) -> yew::Html {
    match resp {
        None => {
            html! { <article aria-busy="true" /> }
        }
        Some(Err(e)) => {
            html! {
                <article class="pico-background-red">
                    { format!("Error fetching data: {}", e) }
                </article>
            }
        }
        Some(Ok(r)) => f(r),
    }
}
