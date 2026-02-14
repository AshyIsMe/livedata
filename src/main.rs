use anyhow::Result;
use clap::Parser;
use livedata::app_controller::ApplicationController;
use livedata::config::Settings;
use livedata::web_server::run_web_server;
use std::thread;
use tracing::info;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

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

    /// Process collection interval in seconds
    #[arg(short = 'p', long, default_value = "5")]
    process_interval: u64,

    /// Number of days to retain log data
    #[arg(long)]
    log_retention_days: Option<u32>,

    /// Maximum log database size in GB
    #[arg(long)]
    log_max_size_gb: Option<f64>,

    /// Number of days to retain process metrics
    #[arg(long)]
    process_retention_days: Option<u32>,

    /// Maximum process metrics database size in GB
    #[arg(long)]
    process_max_size_gb: Option<f64>,

    /// Cleanup interval in minutes (5-15, clamped)
    #[arg(long)]
    cleanup_interval: Option<u32>,

    /// Write plaintext SQL trace to data_dir/trace.sql
    #[arg(long)]
    sql_trace: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Parser, Debug)]
enum Commands {
    /// Run the web server
    Web {
        /// Bind web server to all interfaces (0.0.0.0) instead of localhost
        #[arg(long)]
        listen_all: bool,
    },
}

fn main() -> Result<()> {
    // Initialize logging to stdout
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_level(true);

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env().add_directive("info".parse()?))
        .with(fmt_layer)
        .init();

    info!("Starting journald log collector with DuckDB storage");

    // Parse command line arguments
    let args = Args::parse();

    // Load configuration with CLI overrides
    let settings = Settings::load_with_cli_args(
        args.log_retention_days,
        args.log_max_size_gb,
        args.process_retention_days,
        args.process_max_size_gb,
        args.cleanup_interval,
    )?;

    info!("Configuration loaded:");
    info!("  Log retention: {} days", settings.log_retention_days);
    info!("  Log max size: {} GB", settings.log_max_size_gb);
    info!(
        "  Process retention: {} days",
        settings.process_retention_days
    );
    info!("  Process max size: {} GB", settings.process_max_size_gb);
    info!(
        "  Cleanup interval: {} minutes",
        settings.cleanup_interval_minutes
    );

    info!("Using data directory: {}", args.data_dir);
    if args.follow {
        info!("Follow mode enabled: skipping historical data processing");
    }
    if args.sql_trace {
        if let Err(e) = std::fs::create_dir_all(&args.data_dir) {
            eprintln!("Failed to create data directory {}: {}", args.data_dir, e);
        }
        let trace_path = std::path::Path::new(&args.data_dir).join("trace.sql");
        if let Err(e) = livedata::sql_trace::init_sql_trace(&trace_path) {
            eprintln!(
                "Failed to initialize SQL trace at {}: {}",
                trace_path.display(),
                e
            );
        } else {
            info!("SQL trace enabled at: {}", trace_path.display());
        }
    }

    // Check if the web subcommand is present
    if let Some(Commands::Web { listen_all }) = args.command {
        let settings_for_web = settings.clone();
        // Create and run the application in the main thread
        let mut app = ApplicationController::new(&args.data_dir, args.process_interval, settings)?;

        // Get shutdown signal to share with web server
        let shutdown_signal = app.get_shutdown_signal();

        // Get process monitor from app BEFORE moving app
        let process_monitor = app.get_process_monitor();
        let buffer = app.get_buffer();

        // Run the web server in a separate thread
        let data_dir = args.data_dir.clone();
        let web_server_handle = thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(run_web_server(
                &data_dir,
                buffer,
                shutdown_signal,
                process_monitor,
                settings_for_web,
                listen_all,
            ));
        });

        app.run(args.follow, false)?;

        // Wait for the web server to finish
        web_server_handle.join().unwrap();

        // Ensure checkpoint after the web server releases its connection.
        app.checkpoint_database();
    } else {
        // Create and run the application in the main thread
        let mut app = ApplicationController::new(&args.data_dir, args.process_interval, settings)?;
        app.run(args.follow, true)?;
    }

    info!("Application shutdown complete");
    Ok(())
}
