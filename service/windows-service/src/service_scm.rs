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
pub const SERVICE_START_TYPE: ServiceStartType = ServiceStartType::AutoStart;

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

/// Installs ZonDPI as a Windows Service in the SCM with Automatic startup.
/// If the service is already installed (e.g. during upgrade or reinstall),
/// its configuration is updated to ensure SERVICE_AUTO_START and current binary path.
pub fn install_service(custom_exe_path: Option<&Path>) -> Result<(), ServiceManagementError> {
    let current_exe = match custom_exe_path {
        Some(p) => p.to_path_buf(),
        None => std::env::current_exe()?,
    };

    let manager = ServiceManager::local_computer(
        None::<&str>,
        ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
    )?;

    info!(exe = %current_exe.display(), "Registering ZonDPI service in Windows SCM (AUTO_START)");

    let service_info = ServiceInfo {
        name: SERVICE_NAME.into(),
        display_name: SERVICE_DISPLAY_NAME.into(),
        service_type: ServiceType::OWN_PROCESS,
        start_type: SERVICE_START_TYPE,
        error_control: ServiceErrorControl::Normal,
        executable_path: current_exe,
        launch_arguments: vec![],
        dependencies: vec![],
        account_name: None,
        account_password: None,
    };

    match manager.create_service(&service_info, ServiceAccess::CHANGE_CONFIG) {
        Ok(service) => {
            let _ = service.set_description(SERVICE_DESCRIPTION);
            info!(
                "Service '{}' successfully registered with AUTO_START",
                SERVICE_NAME
            );
        }
        Err(windows_service::Error::Winapi(ref io_err)) if io_err.raw_os_error() == Some(1073) => {
            // ERROR_SERVICE_EXISTS (1073 / 0x431) - Service already registered; update config
            info!(
                "Service '{}' already exists in SCM; reconfiguring to AUTO_START",
                SERVICE_NAME
            );
            let service = manager.open_service(SERVICE_NAME, ServiceAccess::CHANGE_CONFIG)?;
            service.change_config(&service_info)?;
            let _ = service.set_description(SERVICE_DESCRIPTION);
            info!(
                "Service '{}' configuration successfully updated to AUTO_START",
                SERVICE_NAME
            );
        }
        Err(e) => return Err(e.into()),
    }

    Ok(())
}

/// Uninstalls ZonDPI service from Windows SCM.
pub fn uninstall_service() -> Result<(), ServiceManagementError> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;

    let service = match manager.open_service(
        SERVICE_NAME,
        ServiceAccess::STOP | ServiceAccess::DELETE | ServiceAccess::QUERY_STATUS,
    ) {
        Ok(s) => s,
        Err(windows_service::Error::Winapi(ref io_err)) if io_err.raw_os_error() == Some(1060) => {
            // ERROR_SERVICE_DOES_NOT_EXIST (1060 / 0x424) - Idempotent removal
            info!(
                "Service '{}' does not exist in SCM, nothing to uninstall",
                SERVICE_NAME
            );
            return Ok(());
        }
        Err(e) => return Err(e.into()),
    };

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

    // Ensure any active adapter DNS override is completely restored on uninstall
    if let Ok(paths) = crate::runtime_paths::RuntimePaths::discover() {
        let dns_ctrl = zondpi_dns::DnsCompatibilityController::new(&paths.data_root);
        let _ = dns_ctrl.restore_dns();
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_startup_type_is_auto_start() {
        assert_eq!(SERVICE_START_TYPE, ServiceStartType::AutoStart);
        // SCM constant SERVICE_AUTO_START = 2 (not SERVICE_DEMAND_START = 3)
        assert_eq!(SERVICE_START_TYPE.to_raw(), 2);
    }

    #[test]
    fn test_service_constants_integrity() {
        assert_eq!(SERVICE_NAME, "ZonDPI");
        assert_eq!(SERVICE_DISPLAY_NAME, "ZonDPI Service");
        assert!(!SERVICE_DESCRIPTION.is_empty());
        assert!(SERVICE_DESCRIPTION.contains("DPI"));
    }
}
