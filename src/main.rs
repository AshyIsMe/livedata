use anyhow::Result;
use clap::Parser;
use livedata::app_controller::ApplicationController;
use livedata::web_server::run_web_server;
use log::info;
use std::thread;

/// livedata - Journald log collector with DuckDB storage
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Data directory for storing DuckDB database
    #[arg(short, long, default_value = "./data")]
    data_dir: String,

    /// Follow mode: don't process historical data, just start following from now
    #[arg(short = 'f', long)]
    follow: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Parser, Debug)]
enum Commands {
    /// Run the web server
    Web,
}

fn main() -> Result<()> {
    // Initialize logging
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    info!("Starting journald log collector with DuckDB storage");

    // Parse command line arguments
    let args = Args::parse();

    info!("Using data directory: {}", args.data_dir);
    if args.follow {
        info!("Follow mode enabled: skipping historical data processing");
    }

    // Check if the web subcommand is present
    if let Some(Commands::Web) = args.command {
        // Run the web server in a separate thread
        let data_dir = args.data_dir.clone();
        let web_server_handle = thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(run_web_server(&data_dir));
        });

        // Create and run the application in the main thread
        let mut app = ApplicationController::new(&args.data_dir)?;
        app.run(args.follow)?;

        // Wait for the web server to finish
        web_server_handle.join().unwrap();
    } else {
        // Create and run the application in the main thread
        let mut app = ApplicationController::new(&args.data_dir)?;
        app.run(args.follow)?;
    }

    info!("Application shutdown complete");
    Ok(())
}
