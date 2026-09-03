//! Windows Service Host integrating with Windows SCM lifecycle events.

use std::ffi::OsString;
use std::time::Duration;
use tracing::{error, info};
use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
};

use crate::engine_mgr::EngineManager;
use crate::ipc_server::IpcServer;
use crate::runtime_paths::RuntimePaths;
use crate::service_scm::SERVICE_NAME;

define_windows_service!(ffi_service_main, service_main);

/// Starts the Windows Service dispatcher.
pub fn run_service() -> Result<(), windows_service::Error> {
    service_dispatcher::start(SERVICE_NAME, ffi_service_main)
}

fn service_main(_arguments: Vec<OsString>) {
    if let Err(e) = run_service_internal() {
        error!(error = %e, "ZonDPI Windows Service failed");
    }
}

fn run_service_internal() -> Result<(), Box<dyn std::error::Error>> {
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel::<()>(1);

    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                info!("SCM requested service stop/shutdown");
                let _ = shutdown_tx.blocking_send(());
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;

    // Report StartPending
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::StartPending,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 1,
        wait_hint: Duration::from_secs(5),
        process_id: None,
    })?;

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    let paths = RuntimePaths::discover()?;
    let engine_mgr = EngineManager::new(paths);
    let ipc_server = IpcServer::default_endpoint(engine_mgr.clone());

    // Report Running
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    info!("ZonDPI Service is now running in SCM");

    rt.block_on(async {
        let server = ipc_server.clone();
        tokio::spawn(async move {
            let _ = server.run().await;
        });

        // Wait for SCM Stop signal
        shutdown_rx.recv().await;

        info!("Commencing service shutdown procedure");
        let _ = status_handle.set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::StopPending,
            controls_accepted: ServiceControlAccept::empty(),
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 1,
            wait_hint: Duration::from_secs(5),
            process_id: None,
        });

        ipc_server.stop();
        let _ = engine_mgr.stop_engine().await;
    });

    // Report Stopped
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    info!("ZonDPI Service stopped cleanly");
    Ok(())
}
