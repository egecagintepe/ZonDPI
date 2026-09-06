//! ZonDPI Windows Service & Daemon Entrypoint.

use clap::{Parser, Subcommand};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub mod engine_adapter;
pub mod engine_mgr;
pub mod hosts;
pub mod ipc_server;
pub mod runtime_paths;
pub mod service_scm;
pub mod supervisor;

use runtime_paths::RuntimePaths;

#[derive(Parser, Debug)]
#[command(
    name = "zondpi-service",
    version,
    about = "ZonDPI Windows Service Backend Daemon"
)]
struct Cli {
    /// Run the service in foreground console development mode (without Windows SCM)
    #[arg(long, short = 'f')]
    foreground: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run the service in foreground console mode
    Run,
    /// Install ZonDPI as a Windows Service in SCM
    Install,
    /// Uninstall ZonDPI from Windows SCM
    Uninstall,
    /// Start the installed Windows Service via SCM
    Start,
    /// Stop the running Windows Service via SCM
    Stop,
    /// Query SCM status of the installed Windows Service
    Status,
    /// Stop running worker processes and remove WinDivert kernel driver
    CleanDriver,
    /// Display version information
    Version,
}

fn init_logging() {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,zondpi=debug"));

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_thread_ids(false);

    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .try_init();
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_logging();

    let args: Vec<String> = std::env::args().collect();

    // If no arguments provided, Windows SCM invokes the binary directly
    if args.len() <= 1 {
        info!("Launching ZonDPI as Windows Service under SCM");
        if let Err(e) = hosts::run_service() {
            eprintln!("Failed to run as Windows Service: {}", e);
            eprintln!(
                "Hint: To run in development console mode, use 'zondpi-service.exe --foreground'"
            );
            std::process::exit(1);
        }
        return Ok(());
    }

    let cli = Cli::parse();

    if cli.foreground {
        return run_foreground_mode();
    }

    match cli.command {
        Some(Commands::Run) => run_foreground_mode(),
        Some(Commands::Install) => {
            service_scm::install_service(None)?;
            println!("Service 'ZonDPI' installed successfully.");
            Ok(())
        }
        Some(Commands::Uninstall) => {
            service_scm::uninstall_service()?;
            println!("Service 'ZonDPI' uninstalled successfully.");
            Ok(())
        }
        Some(Commands::Start) => {
            service_scm::start_service()?;
            println!("Service 'ZonDPI' start command issued.");
            Ok(())
        }
        Some(Commands::Stop) => {
            service_scm::stop_service()?;
            println!("Service 'ZonDPI' stop command issued.");
            Ok(())
        }
        Some(Commands::Status) => {
            let status = service_scm::query_status()?;
            println!("Service 'ZonDPI' Status: {:?}", status.current_state);
            Ok(())
        }
        Some(Commands::CleanDriver) => {
            service_scm::cleanup_windivert_driver()?;
            println!("WinDivert kernel driver and worker processes cleaned up successfully.");
            Ok(())
        }
        Some(Commands::Version) => {
            println!("zondpi-service version {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        None => run_foreground_mode(),
    }
}

fn run_foreground_mode() -> Result<(), Box<dyn std::error::Error>> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    let paths = RuntimePaths::discover()?;
    rt.block_on(hosts::run_foreground(paths))
}
