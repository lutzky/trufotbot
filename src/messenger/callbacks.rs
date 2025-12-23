use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum Action {
    Take {
        patient_id: i64,
        medication_id: i64,
        quantity: f64,
    },
    Link {
        url: url::Url,
    },
}
