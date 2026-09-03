//! Foreground console execution host for development, testing, and debugging without SCM.

use tracing::info;

use crate::engine_mgr::EngineManager;
use crate::ipc_server::IpcServer;
use crate::runtime_paths::RuntimePaths;

/// Runs the ZonDPI runtime in the foreground as a console application.
pub async fn run_foreground(runtime_paths: RuntimePaths) -> Result<(), Box<dyn std::error::Error>> {
    info!("==================================================");
    info!("   Starting ZonDPI Backend (Foreground Dev Mode)  ");
    info!("==================================================");
    info!("Install Root: {}", runtime_paths.install_root.display());
    info!("Data Root:    {}", runtime_paths.data_root.display());

    let engine_mgr = EngineManager::new(runtime_paths);
    let ipc_server = IpcServer::default_endpoint(engine_mgr.clone());

    let server_clone = ipc_server.clone();
    let server_handle = tokio::spawn(async move {
        if let Err(e) = server_clone.run().await {
            tracing::error!(error = %e, "IPC server encountered error");
        }
    });

    info!("ZonDPI Backend initialized. Press Ctrl+C to terminate.");

    // Await Ctrl+C signal
    tokio::signal::ctrl_c().await?;

    info!("Shutdown signal received. Commencing graceful teardown...");
    ipc_server.stop();
    let _ = engine_mgr.stop_engine().await;
    let _ = server_handle.await;

    info!("ZonDPI Backend teardown complete.");
    Ok(())
}
