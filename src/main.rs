use std::{str::FromStr, sync::Arc};

use anyhow::Result;
use app_state::{AppState, Config};
use axum_embed::ServeEmbed;
use clap::{Parser, Subcommand};
use messenger::{nil_sender::NilSender, telegram_sender::TelegramSender};
use rust_embed::RustEmbed;

use dotenv::dotenv;
use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};
use teloxide::Bot;
use utoipa::OpenApi;
use utoipa_axum::{router::OpenApiRouter, routes};

mod api;
mod app_state;
mod autocomplete;
mod dose_limits;
mod errors;
mod handlers;
mod messenger;
mod models;
mod next_doses;
mod reminder_scheduler;
mod seed;
mod storage;
mod telegram_bot;
mod time;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Output OpenAPI Schema
    Schema,

    /// Seed the database
    Seed {
        // This is an arg because it's often a negative number, and the initial minus gets confused
        // for a separate argument. Use like so: -g=-12345
        #[arg(short = 'g', long)]
        telegram_group_id: Option<i64>,
    },

    Serve {
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        #[arg(long, default_value_t = 3000)]
        port: u16,
    },
}

#[derive(RustEmbed, Clone)]
#[folder = "assets/"]
#[exclude = ".gitignore"]
struct Assets;

#[derive(OpenApi)]
#[openapi(
    tags(
        (name = handlers::status::UTOIPA_TAG, description = "Status API"),
        (name = handlers::doses::UTOIPA_TAG, description = "Doses API"),
        (name = handlers::medication::UTOIPA_TAG, description = "Medication API"),
        (name = handlers::patients::UTOIPA_TAG, description = "Patients API"),
        (name = handlers::reminders::UTOIPA_TAG, description = "Reminders API"),
    ),
)]
struct ApiDoc;

#[tokio::main]
#[allow(clippy::unwrap_used)]
async fn main() -> Result<()> {
    let args = Args::parse();
    dotenv().ok(); // Load .env file

    pretty_env_logger::init_timed();

    log::info!("Starting the server...");

    let config = Config::load()?;

    let connect_options =
        SqliteConnectOptions::from_str(&config.database_url)?.create_if_missing(true);
    let pool = SqlitePool::connect_with(connect_options).await?;

    // Run migrations on startup (optional, but good for development)
    sqlx::migrate!().run(&pool).await?;

    match &args.command {
        Commands::Seed { telegram_group_id } => {
            log::info!("Seeding database...");
            seed::seed_database(&pool, *telegram_group_id).await?;
            log::info!("Database seeded successfully!");
            Ok(())
        }
        Commands::Serve { host, port } => serve(host, *port, pool, config).await,
        Commands::Schema => {
            let app_state = AppState::new(pool, NilSender::new().into(), Arc::new(config))
                .await
                .unwrap();
            let (_, openapi) = app_router(app_state).split_for_parts();
            println!("{}", openapi.to_pretty_json()?);
            Ok(())
        }
    }
}

fn app_router(state: AppState) -> OpenApiRouter {
    OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(handlers::status::get))
        .routes(routes!(
            handlers::patients::list,
            handlers::patients::create
        ))
        .routes(routes!(
            handlers::patients::get,
            handlers::patients::update,
            handlers::patients::delete
        ))
        .routes(routes!(handlers::medication::create))
        .routes(routes!(handlers::medication::delete))
        .routes(routes!(handlers::medication::update))
        .routes(routes!(handlers::doses::list, handlers::doses::record))
        .routes(routes!(
            handlers::doses::get,
            handlers::doses::update,
            handlers::doses::delete
        ))
        .routes(routes!(handlers::reminders::send_reminder))
        .routes(routes!(handlers::reminders::get, handlers::reminders::set))
        .with_state(state)
}

async fn serve(
    host: &str,
    port: u16,
    pool: sqlx::Pool<sqlx::Sqlite>,
    config: Config,
) -> Result<()> {
    let config = Arc::new(config);

    let bot = if std::env::var("TELOXIDE_TOKEN").is_ok() {
        let bot = Bot::from_env();

        Some(bot)
    } else {
        log::warn!("TELOXIDE_TOKEN not set, Telegram bot functionality will be disabled.");
        None
    };

    let messenger = match bot.clone() {
        Some(bot) => TelegramSender::new(bot).into(),
        None => NilSender::new().into(),
    };

    let app_state = AppState::new(pool, messenger, config.clone()).await?;

    if let Some(bot) = bot {
        let bot = bot.clone();
        let storage = app_state.storage.clone();

        tokio::spawn(async move { telegram_bot::launch(bot, storage, config).await });
    }

    let serve_assets = ServeEmbed::<Assets>::with_parameters(
        // Return index.html for any path; that'll hit yew's BrowserRouter and
        // let it handle the routing.
        Some("index.html".to_owned()),
        axum_embed::FallbackBehavior::Ok,
        None,
    );

    let (router, api_doc) = app_router(app_state.clone()).split_for_parts();

    let app = router.fallback_service(serve_assets).merge(
        utoipa_swagger_ui::SwaggerUi::new("/swagger-ui").url("/api-doc/openapi.json", api_doc),
    );

    app_state
        .clone()
        .reminder_scheduler
        .set_reminders_from_db(&app_state.storage.pool.clone())
        .await?;

    let listener = tokio::net::TcpListener::bind((host, port)).await?;
    log::info!("Listening on {}", listener.local_addr()?);
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

#[allow(clippy::expect_used)]
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
