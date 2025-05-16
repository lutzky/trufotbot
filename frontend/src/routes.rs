use yew_router::prelude::*;

#[derive(Clone, Routable, PartialEq)]
pub enum Route {
    #[at("/")]
    Home,
    #[at("/patients/:id")]
    PatientDetail { id: i64 },
    #[at("/patients/:patient_id/medications/:medication_id")]
    PatientMedicationDetail { patient_id: i64, medication_id: i64 },
    #[at("/patients/:patient_id/medications/:medication_id/doses/:dose_id")]
    DoseEdit {
        patient_id: i64,
        medication_id: i64,
        dose_id: i64,
    },
    #[not_found]
    #[at("/404")]
    NotFound, // A catch-all for invalid URLs
}
