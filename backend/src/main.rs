// SPDX-FileCopyrightText: 2023-2026 Sayantan Santra <sayantan.santra689@gmail.com>
// SPDX-License-Identifier: MIT

use log::info;
use rusqlite::Connection;
use std::{
    io::Result,
    sync::{Arc, Once},
};
use tokio::sync::{Mutex, mpsc};
use tower::Layer;

// Import modules
mod auth;
mod background;
mod config;
mod database;
mod services;

use services::utils;

// Tests
#[cfg(test)]
mod tests;

// This struct represents state
struct AppState {
    hits_tx: mpsc::Sender<(String, bool)>,
    // Wrapped in a Mutex because rusqlite's Connection is !Sync, and Axum
    // handlers require Send + Sync shared state.
    reader: Mutex<Connection>,
    writer: Arc<Mutex<Connection>>,
    config: config::Config,
}

static LOGGER: Once = Once::new();

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::builder()
        .parse_filters(
            std::env::var("RUST_LOG")
                .ok()
                .filter(|s| !s.is_empty())
                .unwrap_or("warn,chhoto_url=info,tower_http=info".to_owned())
                .as_str(),
        )
        .format(|buf, record| {
            use chrono::Local;
            use env_logger::fmt::style::{AnsiColor, Style};
            use std::io::Write;

            let subtle = Style::new().fg_color(Some(AnsiColor::BrightBlack.into()));
            let level_style = buf.default_level_style(record.level());

            writeln!(
                buf,
                "{subtle}[{subtle:#}{} {level_style}{:<6}{level_style:#}{}{subtle}]{subtle:#} {}",
                Local::now().format("%Y-%m-%d %H:%M:%S%Z"),
                record.level(),
                record.module_path().unwrap_or_default(),
                record.args()
            )
        })
        .init();

    eprintln!("----------------------------------------------------------------------");
    info!("Starting Chhoto URL Server v{}", utils::get_version());
    info!("Source: https://github.com/SinTan1729/chhoto-url");
    eprintln!("----------------------------------------------------------------------");

    // Read config from env vars
    let conf = config::read();
    // ArcMutex is necessary since the writer is shared across threads
    let writer = Arc::new(Mutex::new(database::open_db(&conf.db_location, false)));

    // Initialize the database and perform migrations
    let use_wal_mode = conf.use_wal_mode;
    database::init_db(&mut *writer.lock().await, use_wal_mode, conf.ensure_acid);
    // Spawn cleaner
    background::spawn_cleaner(Arc::clone(&writer), use_wal_mode);
    // Spawn hit updater
    let (hits_tx, hits_rx) = mpsc::channel::<(String, bool)>(1024);
    background::spawn_hits_worker(Arc::clone(&writer), hits_rx);

    let port = conf.port;
    let addr = conf.listen_address.clone();

    // Maintain a single instance of state throughout
    let state = Arc::new(AppState {
        hits_tx,
        reader: Mutex::new(database::open_db(&conf.db_location, true)),
        writer: Arc::clone(&writer),
        config: conf.clone(),
    });

    // Build the router (routes + tower middleware + session layer)
    let app = services::build_router(state, &conf);
    // NormalizePath must wrap the whole router so it runs before routing.
    let app = tower_http::normalize_path::NormalizePathLayer::trim_trailing_slash().layer(app);

    // Hardcode the port the server listens to. Allows for more intuitive Docker Compose port management
    let listener = tokio::net::TcpListener::bind((addr.as_str(), port)).await?;
    LOGGER.call_once(|| {
        info!(
            "Server has started listening to {} on port {}.",
            &addr, port
        );
    });

    axum::serve(
        listener,
        axum::ServiceExt::<axum::extract::Request>::into_make_service(app),
    )
    .await
}
