use std::str::FromStr;

use app_state::AppState;
use axum::Router;
use axum_embed::ServeEmbed;
use clap::Parser;
use rust_embed::RustEmbed;

use dotenv::dotenv;
use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};
use teloxide::Bot;

mod app_state;
mod dose_limits;
mod handlers;
mod messenger;
mod models;
mod next_doses;
mod reminder_scheduler;
mod seed;
mod storage;
mod telegram_bot;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(long)]
    seed: bool,

    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    #[arg(long, default_value_t = 3000)]
    port: u16,
}

#[derive(RustEmbed, Clone)]
#[folder = "assets/"]
#[exclude = ".gitignore"]
struct Assets;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use axum::routing::{delete, get, post, put};

    let args = Args::parse();
    dotenv().ok(); // Load .env file

    pretty_env_logger::init();

    log::info!("Starting the server...");

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL not set");
    let connect_options = SqliteConnectOptions::from_str(&database_url)?.create_if_missing(true);
    let pool = SqlitePool::connect_with(connect_options).await?;

    // Run migrations on startup (optional, but good for development)
    sqlx::migrate!().run(&pool).await?;

    if args.seed {
        log::info!("Seeding database...");
        seed::seed_database(&pool).await?;
        log::info!("Database seeded successfully!");
        return Ok(());
    }

    let telegram_bot = if std::env::var("TELOXIDE_TOKEN").is_ok() {
        let bot = Bot::from_env();

        Some(bot)
    } else {
        log::warn!("TELOXIDE_TOKEN not set, Telegram bot functionality will be disabled.");
        None
    };

    let app_state = AppState::new(pool, telegram_bot).await?;

    let serve_assets = ServeEmbed::<Assets>::with_parameters(
        // Return index.html for any path; that'll hit yew's BrowserRouter and
        // let it handle the routing.
        Some("index.html".to_owned()),
        axum_embed::FallbackBehavior::Ok,
        None,
    );

    // Build the Axum application
    let app = Router::new()
        .fallback_service(serve_assets)
        .route("/api/medications", post(handlers::medication::create))
        .route(
            "/api/medications/{medication_id}",
            delete(handlers::medication::delete),
        )
        .route("/api/patients", get(handlers::patients::list))
        .route("/api/patients", post(handlers::patients::create))
        .route("/api/patients/{patient_id}", get(handlers::patients::get))
        .route(
            "/api/patients/{patient_id}",
            put(handlers::patients::update),
        )
        .route(
            "/api/patients/{patient_id}",
            delete(handlers::patients::delete),
        )
        .route(
            "/api/patients/{patient_id}/ping",
            post(handlers::patients::ping),
        )
        .route(
            "/api/patients/{patient_id}/medications/{medication_id}",
            put(handlers::medication::update),
        )
        .route(
            "/api/patients/{patient_id}/medications/{medication_id}/doses",
            get(handlers::doses::list),
        )
        .route(
            "/api/patients/{patient_id}/medications/{medication_id}/doses",
            post(handlers::doses::record),
        )
        .route(
            "/api/patients/{patient_id}/medications/{medication_id}/doses/{dose_id}",
            get(handlers::doses::get),
        )
        .route(
            "/api/patients/{patient_id}/medications/{medication_id}/doses/{dose_id}",
            put(handlers::doses::update),
        )
        .route(
            "/api/patients/{patient_id}/medications/{medication_id}/doses/{dose_id}",
            delete(handlers::doses::delete),
        )
        .route(
            "/api/patients/{patient_id}/medications/{medication_id}/remind",
            put(handlers::reminders::send_reminder),
        )
        .route(
            "/api/patients/{patient_id}/medications/{medication_id}/reminders",
            get(handlers::reminders::get),
        )
        .route(
            "/api/patients/{patient_id}/medications/{medication_id}/reminders",
            put(handlers::reminders::set),
        )
        .with_state(app_state.clone());

    app_state
        .clone()
        .reminder_scheduler
        .set_reminders_from_db(&app_state.storage.pool.clone())
        .await?;

    let listener = tokio::net::TcpListener::bind((args.host, args.port)).await?;
    log::info!("Listening on {}", listener.local_addr()?);
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    // Doing this manually is required for running in Docker, as PID=1 processes
    // must handle SIGTERM explicitly.
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
        log::info!("Ctrl+C received, shutting down...");
    };

    let terminal = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
        log::info!("SIGTERM received, shutting down...");
    };

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminal => {},
    }
}
