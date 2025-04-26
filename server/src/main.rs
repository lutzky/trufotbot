use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json, Router,
};

use axum_embed::ServeEmbed;
use rust_embed::RustEmbed;
use teloxide::prelude::*;

// cspell: words sqlx dotenv chrono teloxide

use chrono::{NaiveDateTime, Utc};
use dotenv::dotenv;
use sqlx::{FromRow, SqlitePool};
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer; // For CORS

#[cfg(test)]
use std::collections::HashMap;

mod models;
use models::{CreateDose, Patient, UserMedicineDetails};

#[derive(Clone)]
struct AppState {
    db: SqlitePool,
    telegram_bot: Option<teloxide::Bot>,

    #[cfg(test)]
    telegram_messages: Arc<Mutex<HashMap<i64, Vec<(i32, String)>>>>,
}

// TODO: Probably rename this to something clearer
trait MessageWithId {
    fn id(&self) -> i32;
}

impl MessageWithId for teloxide::types::Message {
    fn id(&self) -> i32 {
        self.id.0
    }
}

impl MessageWithId for i32 {
    fn id(&self) -> i32 {
        *self
    }
}

impl AppState {
    fn new(db: SqlitePool, telegram_bot: Option<teloxide::Bot>) -> Self {
        AppState {
            db,
            telegram_bot,

            #[cfg(test)]
            telegram_messages: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn send_message(
        &self,
        patient: &Patient,
        message: String,
    ) -> Result<Option<impl MessageWithId>, Box<dyn std::error::Error>> {
        #[cfg(test)]
        {
            self.send_message_mock(patient, message).await
        }
        #[cfg(not(test))]
        {
            self.send_message_telegram(patient, message).await
        }
    }

    async fn send_message_telegram(
        &self,
        patient: &Patient,
        message: String,
    ) -> Result<Option<Message>, Box<dyn std::error::Error>> {
        let Some(telegram_group_id) = patient.telegram_group_id else {
            log::warn!(
                "Patient {} has no telegram group ID, skipping message.",
                patient.name
            );
            return Ok(None);
        };

        let Some(bot) = &self.telegram_bot else {
            log::warn!("Telegram bot is not configured, skipping message.");
            return Ok(None);
        };

        let message = bot.send_message(ChatId(telegram_group_id), message).await?;

        Ok(Some(message))
    }

    #[cfg(test)]
    async fn send_message_mock(
        &self,
        patient: &Patient,
        message: String,
    ) -> Result<Option<impl MessageWithId>, Box<dyn std::error::Error>> {
        let Some(telegram_group_id) = patient.telegram_group_id else {
            log::warn!(
                "Patient {} has no telegram group ID, skipping message.",
                patient.name
            );
            return Ok(None);
        };
        let mut telegram_messages = self.telegram_messages.lock().await;

        let messages = telegram_messages
            .entry(telegram_group_id)
            .or_insert_with(Vec::new);

        let id = messages.len() as i32 + 1;

        messages.push((id, message.clone()));

        Ok(Some(id))
    }
}

#[derive(RustEmbed, Clone)]
#[folder = "assets/"]
#[exclude = ".gitignore"]
struct Assets;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use axum::routing::{get, patch, post, put};
    dotenv().ok(); // Load .env file

    pretty_env_logger::init();

    log::info!("Starting the server...");

    let telegram_bot = if std::env::var("TELOXIDE_TOKEN").is_ok() {
        Some(Bot::from_env())
    } else {
        log::warn!("TELOXIDE_TOKEN not set, Telegram bot functionality will be disabled.");
        None
    };

    // Set up the database connection pool
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL not set");
    let pool = SqlitePool::connect(&database_url).await?;

    // Run migrations on startup (optional, but good for development)
    sqlx::migrate!().run(&pool).await?;

    // TODO: Initialize Telegram bot client here

    let app_state = AppState::new(pool, telegram_bot);

    let serve_assets = ServeEmbed::<Assets>::new();

    // Build the Axum application
    let app = Router::new()
        .fallback_service(serve_assets)
        .route("/api/patients", get(list_patients))
        .route("/api/patients/{patient_id}", patch(update_patient))
        .route("/api/patients/{patient_id}/ping", post(ping_patient))
        .route(
            "/api/patients/{patient_id}/doses/{medication_id}",
            put(record_dose),
        )
        // TODO: There's some kind of standard for how to name these - https://stackoverflow.blog/2020/03/02/best-practices-for-rest-api-design/
        // .route(
        //     "/patients/:patient_id/medications",
        //     get(list_patient_medications),
        // )
        // .route(
        //     "/patients/:patient_id/medications/:medication_id",
        //     get(get_user_medicine_details),
        // )
        // .route(
        //     "/patients/:patient_id/medications/:medication_id",
        //     post(record_dose),
        // )
        .layer(CorsLayer::permissive()) // Allow all origins for simplicity during development // FIXME?
        .with_state(app_state);

    // Run the server
    // TODO: Make listening bind flag-configurable
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    log::info!("Listening on {}", listener.local_addr()?);
    axum::serve(listener, app).await?;

    Ok(())
}

// --- API Handlers ---

async fn list_patients(
    State(app_state): State<AppState>,
) -> Result<Json<Vec<Patient>>, (StatusCode, String)> {
    let patients = sqlx::query_as!(Patient, "SELECT id, name, telegram_group_id FROM patients")
        .fetch_all(&app_state.db)
        .await
        .map_err(|e| {
            log::error!("Database error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to fetch users".to_string(),
            )
        })?;

    Ok(Json(patients))
}

async fn ping_patient(
    State(app_state): State<AppState>,
    Path(patient_id): Path<i64>,
) -> Result<StatusCode, (StatusCode, String)> {
    let patient = sqlx::query_as!(
        Patient,
        r"SELECT id, name, telegram_group_id FROM patients WHERE id = ?",
        patient_id
    )
    .fetch_optional(&app_state.db)
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to fetch patient".to_string(),
        )
    })?;

    let Some(patient) = patient else {
        return Err((StatusCode::NOT_FOUND, "Patient not found".to_string()));
    };

    log::debug!("Pinging patient {:?}", patient);

    app_state
        .send_message(&patient, "Ping!".to_string())
        .await
        .map_err(|e| {
            log::error!("Telegram error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to send message".to_string(),
            )
        })?;

    Ok(StatusCode::OK)
}

#[derive(serde::Deserialize)]
struct UpdatePatient {
    name: Option<String>,
    telegram_group_id: Option<i64>,
}

async fn update_patient(
    State(app_state): State<AppState>,
    Path(patient_id): Path<i64>,
    Json(payload): Json<UpdatePatient>,
) -> Result<StatusCode, (StatusCode, String)> {
    let result = sqlx::query!(
        r#"
        UPDATE patients
        SET name = COALESCE(?, name),
            telegram_group_id = COALESCE(?, telegram_group_id)
        WHERE id = ?
        "#,
        payload.name,
        payload.telegram_group_id,
        patient_id
    )
    .execute(&app_state.db)
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to update patient".to_string(),
        )
    })?;
    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Patient not found".to_string()));
    }
    Ok(StatusCode::OK)
}

async fn list_patient_medications(
    Path(user_id): Path<i64>,
    State(app_state): State<AppState>,
) -> Result<Json<Vec<UserMedicineDetails>>, (StatusCode, String)> {
    let medicines = sqlx::query_as::<_, UserMedicineDetails>(
        r#"
        SELECT
            um.user_id,
            um.medicine_id,
            m.name AS medicine_name,
            (SELECT taken_at FROM intake_records WHERE user_id = um.user_id AND medicine_id = um.medicine_id ORDER BY taken_at DESC LIMIT 1) AS last_taken_at,
            um.daily_reminder_hour,
            um.telegram_group_id,
            um.default_quantity
        FROM user_medicines um
        JOIN medicines m ON um.medicine_id = m.id
        WHERE um.user_id = ?
        "#
    )
    .bind(user_id)
    .fetch_all(&app_state.db)
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch user medicines".to_string())
    })?;

    Ok(Json(medicines))
}

async fn get_user_medicine_details(
    Path((user_id, medicine_id)): Path<(i64, i64)>,
    State(app_state): State<AppState>,
) -> Result<Json<UserMedicineDetails>, (StatusCode, String)> {
    let details = sqlx::query_as::<_, UserMedicineDetails>(
        r#"
        SELECT
            um.user_id,
            um.medicine_id,
            m.name AS medicine_name,
            (SELECT taken_at FROM intake_records WHERE user_id = um.user_id AND medicine_id = um.medicine_id ORDER BY taken_at DESC LIMIT 1) AS last_taken_at,
            um.daily_reminder_hour,
            um.telegram_group_id,
            um.default_quantity
        FROM user_medicines um
        JOIN medicines m ON um.medicine_id = m.id
        WHERE um.user_id = ? AND um.medicine_id = ?
        "#
    )
    .bind(user_id)
    .bind(medicine_id)
    .fetch_one(&app_state.db)
    .await
    .map_err(|e| match e {
            sqlx::Error::RowNotFound => (StatusCode::NOT_FOUND, "User medicine details not found".to_string()),
            _ => {
                log::error!("Database error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch user medicine details".to_string())
            }
        })?;

    Ok(Json(details))
}

async fn record_dose(
    Path((patient_id, medication_id)): Path<(i64, i64)>,
    State(app_state): State<AppState>,
    Json(payload): Json<CreateDose>,
) -> Result<StatusCode, (StatusCode, String)> {
    let patient = sqlx::query_as!(
        Patient,
        "SELECT id, name, telegram_group_id FROM patients WHERE id = ?",
        patient_id
    )
    .fetch_optional(&app_state.db)
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to fetch patient".to_string(),
        )
    })?;

    let Some(patient) = patient else {
        return Err((StatusCode::NOT_FOUND, "Patient not found".to_string()));
    };

    // TODO: Test what happens if the medication_id is not found

    let medication = sqlx::query_as!(
        models::Medication,
        "SELECT id, name, description FROM medications WHERE id = ?",
        medication_id
    )
    .fetch_optional(&app_state.db)
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to fetch medication".to_string(),
        )
    })?;

    let Some(medication) = medication else {
        return Err((StatusCode::NOT_FOUND, "Medication not found".to_string()));
    };

    sqlx::query!(
        r#"
        INSERT INTO  doses (patient_id, medication_id, quantity, taken_at, noted_by_user)
        VALUES (?, ?, ?, ?, ?)
        "#,
        patient_id,
        medication_id,
        payload.quantity,
        payload.taken_at,
        payload.noted_by_user,
    )
    .execute(&app_state.db)
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to record intake".to_string(),
        )
    })?;

    let who_gave_whom = match payload.noted_by_user {
        Some(name) if name == patient.name => format!("{} took {}", name, medication.name),
        Some(name) => format!("{} gave {} {}", name, patient.name, medication.name),
        None => format!(
            "{} was given {} (by unknown)",
            patient.name, medication.name
        ),
    };

    app_state
        .send_message(
            &patient,
            format!(
                "{who_gave_whom} ({}) at ({})",
                payload.quantity, payload.taken_at
            ),
        )
        .await
        .map_err(|e| {
            log::error!("Telegram error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to send message".to_string(),
            )
        })?;

    // TODO: Humanize taken_at

    // TODO: Support editing previous messages instead if this is a result of a reminder

    Ok(StatusCode::CREATED)
}

// TODO: Add endpoint for editing intake records if needed.
// TODO: Add endpoints for managing users and medicines via API (optional for base version)

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[sqlx::test(fixtures("patients"))]
    async fn list_patients_correct(db: SqlitePool) {
        let app_state = AppState::new(db, None);

        let patients = list_patients(State(app_state)).await.unwrap();
        assert_eq!(
            patients.0,
            vec![
                Patient {
                    id: 1,
                    telegram_group_id: Some(-123),
                    name: "Alice".to_string(),
                },
                Patient {
                    id: 2,
                    telegram_group_id: Some(-123),
                    name: "Bob".to_string(),
                },
                Patient {
                    id: 3,
                    telegram_group_id: Some(-123),
                    name: "Carol".to_string(),
                },
            ]
        );
    }

    #[sqlx::test(fixtures("patients"))]
    async fn record_dose_fails_with_nonexistent_medication(db: SqlitePool) {
        let app_state = AppState::new(db, None);

        let result = record_dose(
            Path((1, 999)),
            State(app_state),
            Json(CreateDose {
                quantity: 2.0,
                taken_at: Utc::now().naive_utc(),
                noted_by_user: Some("Alice".to_string()),
            }),
        )
        .await;

        assert_eq!(
            result,
            Err((StatusCode::NOT_FOUND, "Medication not found".to_string()))
        );
    }

    #[sqlx::test(fixtures("patients", "medications"))]
    async fn record_dose_succeeds(db: SqlitePool) {
        let app_state = AppState::new(db, None);

        let taken_at =
            NaiveDateTime::parse_from_str("2023-04-05 06:07:08", "%Y-%m-%d %H:%M:%S").unwrap();

        record_dose(
            Path((1, 1)),
            State(app_state.clone()),
            Json(CreateDose {
                quantity: 2.0,
                taken_at,
                noted_by_user: Some("Alice".to_string()),
            }),
        )
        .await
        .unwrap();

        let result = sqlx::query!(
            r#"SELECT taken_at FROM doses 
              WHERE
                patient_id = 1 AND 
                medication_id = 1 AND
                quantity = 2.0 AND
                noted_by_user = "Alice""#,
        )
        .fetch_one(&app_state.db)
        .await
        .unwrap();

        assert_eq!(result.taken_at, taken_at);

        let telegram_messages = app_state.telegram_messages.lock().await;
        assert_eq!(
            *telegram_messages,
            HashMap::from([(
                -123,
                vec![(
                    1,
                    "Alice took Aspirin (2) at (2023-04-05 06:07:08)".to_string()
                )]
            ),])
        );
    }
}
