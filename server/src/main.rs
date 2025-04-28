use axum::Router;

use axum_embed::ServeEmbed;
use rust_embed::RustEmbed;

// cspell: words sqlx dotenv chrono teloxide

use dotenv::dotenv;
use sqlx::SqlitePool;
use teloxide::Bot;
use tower_http::cors::CorsLayer; // For CORS

mod app_state;
mod handlers;
mod models;
use app_state::AppState;

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
        .route("/api/patients", get(handlers::patient::list))
        .route(
            "/api/patients/{patient_id}",
            get(handlers::patient::get_medication_menu),
        )
        .route(
            "/api/patients/{patient_id}",
            patch(handlers::patient::update),
        )
        .route(
            "/api/patients/{patient_id}/ping",
            post(handlers::patient::ping),
        )
        .route(
            "/api/patients/{patient_id}/doses/{medication_id}",
            put(handlers::patient::doses::record),
        )
        .route(
            "/api/patients/{patient_id}/remind/{medication_id}",
            put(handlers::patient::remind::send_reminder),
        )
        // TODO: There's some kind of standard for how to name these - https://stackoverflow.blog/2020/03/02/best-practices-for-rest-api-design/
        .layer(CorsLayer::permissive()) // Allow all origins for simplicity during development // FIXME?
        .with_state(app_state);

    // Run the server
    // TODO: Make listening bind flag-configurable
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    log::info!("Listening on {}", listener.local_addr()?);
    axum::serve(listener, app).await?;

    Ok(())
}

// TODO: Add endpoint for editing intake records if needed.
// TODO: Add endpoints for managing users and medicines via API (optional for base version)
