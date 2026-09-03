//! Windows Service Control Manager (SCM) integration for install, uninstall, start, stop, and status.

use std::path::Path;
use thiserror::Error;
use tracing::info;
use windows_service::service::{
    ServiceAccess, ServiceErrorControl, ServiceInfo, ServiceStartType, ServiceState, ServiceStatus,
    ServiceType,
};
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

pub const SERVICE_NAME: &str = "ZonDPI";
pub const SERVICE_DISPLAY_NAME: &str = "ZonDPI Service";
pub const SERVICE_DESCRIPTION: &str =
    "Automated DPI circumvention and packet routing backend daemon.";

#[derive(Error, Debug)]
pub enum ServiceManagementError {
    #[error("Windows Service API error: {0}")]
    WindowsService(#[from] windows_service::Error),
    #[error("Failed to determine current executable path: {0}")]
    ExecutablePath(#[from] std::io::Error),
    #[error("Service is already running")]
    AlreadyRunning,
    #[error("Service is not running")]
    NotRunning,
}

/// Installs ZonDPI as a Windows Service in the SCM.
pub fn install_service(custom_exe_path: Option<&Path>) -> Result<(), ServiceManagementError> {
    let current_exe = match custom_exe_path {
        Some(p) => p.to_path_buf(),
        None => std::env::current_exe()?,
    };

    let manager = ServiceManager::local_computer(
        None::<&str>,
        ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
    )?;

    info!(exe = %current_exe.display(), "Registering ZonDPI service in Windows SCM");

    let service_info = ServiceInfo {
        name: SERVICE_NAME.into(),
        display_name: SERVICE_DISPLAY_NAME.into(),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::OnDemand,
        error_control: ServiceErrorControl::Normal,
        executable_path: current_exe,
        launch_arguments: vec![],
        dependencies: vec![],
        account_name: None,
        account_password: None,
    };

    let service = manager.create_service(&service_info, ServiceAccess::empty())?;

    info!("Service '{}' successfully registered", SERVICE_NAME);
    drop(service);
    Ok(())
}

/// Uninstalls ZonDPI service from Windows SCM.
pub fn uninstall_service() -> Result<(), ServiceManagementError> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;

    let service = manager.open_service(
        SERVICE_NAME,
        ServiceAccess::STOP | ServiceAccess::DELETE | ServiceAccess::QUERY_STATUS,
    )?;

    // Stop service first if running
    if let Ok(status) = service.query_status() {
        if status.current_state != ServiceState::Stopped {
            info!("Stopping service prior to deletion");
            let _ = service.stop();
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    }

    info!("Deleting service '{}' from SCM", SERVICE_NAME);
    service.delete()?;
    info!("Service '{}' deleted successfully", SERVICE_NAME);
    Ok(())
}

/// Starts the ZonDPI Windows Service via SCM.
pub fn start_service() -> Result<(), ServiceManagementError> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;

    let service = manager.open_service(
        SERVICE_NAME,
        ServiceAccess::START | ServiceAccess::QUERY_STATUS,
    )?;

    let status = service.query_status()?;
    if status.current_state == ServiceState::Running {
        return Err(ServiceManagementError::AlreadyRunning);
    }

    info!("Starting service '{}' via SCM", SERVICE_NAME);
    let no_args: &[&str] = &[];
    service.start(no_args)?;
    info!("Service start command issued successfully");
    Ok(())
}

/// Stops the ZonDPI Windows Service via SCM.
pub fn stop_service() -> Result<(), ServiceManagementError> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;

    let service = manager.open_service(
        SERVICE_NAME,
        ServiceAccess::STOP | ServiceAccess::QUERY_STATUS,
    )?;

    let status = service.query_status()?;
    if status.current_state == ServiceState::Stopped {
        return Err(ServiceManagementError::NotRunning);
    }

    info!("Stopping service '{}' via SCM", SERVICE_NAME);
    service.stop()?;
    info!("Service stop command issued successfully");
    Ok(())
}

/// Queries current SCM status of the ZonDPI Windows Service.
pub fn query_status() -> Result<ServiceStatus, ServiceManagementError> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;

    let service = manager.open_service(SERVICE_NAME, ServiceAccess::QUERY_STATUS)?;

    let status = service.query_status()?;
    Ok(status)
}
