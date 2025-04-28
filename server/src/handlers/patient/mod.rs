use crate::app_state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use shared::api::patient_types::{PatientMedicationMenu, PatientMedicationMenuItem};

pub async fn get_patient_medication_menu(
    Path(patient_id): Path<i64>,
    State(app_state): State<AppState>,
) -> Result<Json<PatientMedicationMenu>, (StatusCode, String)> {
    // Fetch patient details
    let patient = app_state.get_patient(patient_id).await?;

    // Fetch medications and their last dose time for this patient
    // This query joins medications and doses, grouping by medication
    let medications = sqlx::query_as!(PatientMedicationMenuItem,
        r#"
        SELECT
            m.id AS "id!",
            m.name AS "name!",
            MAX(d.taken_at) AS last_taken_at
        FROM medications m
        LEFT JOIN doses d ON m.id = d.medication_id AND d.patient_id = $1
        GROUP BY m.id, m.name
        ORDER BY last_taken_at DESC NULLS LAST
        "#,
        patient_id
    )
    .fetch_all(&app_state.db)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch medication data".into()))?;

    let response = PatientMedicationMenu {
        patient_id: patient.id,
        patient_name: patient.name,
        medications,
    };

    Ok(Json(response))
}
