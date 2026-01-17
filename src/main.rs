use anyhow::Result;
use livedata::app_controller::ApplicationController;
use log::info;
use std::env;

fn main() -> Result<()> {
    // Initialize logging
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    info!("Starting journald to parquet log collector");

    // Get data directory from command line args or use default
    let data_dir = env::args().nth(1).unwrap_or_else(|| "./data".to_string());

    info!("Using data directory: {}", data_dir);

    // Create and run the application
    let mut app = ApplicationController::new(&data_dir)?;
    app.run()?;

    info!("Application shutdown complete");
    Ok(())
}
