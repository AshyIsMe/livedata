use anyhow::Result;
use clap::Parser;
use livedata::app_controller::ApplicationController;
use log::info;

/// livedata - Journald to parquet log collector
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Data directory for storing parquet files
    #[arg(short, long, default_value = "./data")]
    data_dir: String,

    /// Follow mode: don't process historical data, just start following from now
    #[arg(short = 'f', long)]
    follow: bool,
}

fn main() -> Result<()> {
    // Initialize logging
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    info!("Starting journald to parquet log collector");

    // Parse command line arguments
    let args = Args::parse();

    info!("Using data directory: {}", args.data_dir);
    if args.follow {
        info!("Follow mode enabled: skipping historical data processing");
    }

    // Create and run the application
    let mut app = ApplicationController::new(&args.data_dir)?;
    app.run(args.follow)?;

    info!("Application shutdown complete");
    Ok(())
}
