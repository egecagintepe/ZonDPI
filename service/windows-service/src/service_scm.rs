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

/// Stops the WinDivert kernel driver service if currently running in the Windows kernel.
pub fn stop_windivert_driver_service() {
    let sys_root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string());
    let sys32 = std::path::PathBuf::from(&sys_root).join("System32");

    let driver_names = [
        "windivert",
        "windivert14",
        "windivert22",
        "WinDivert",
        "WinDivert14",
        "WinDivert22",
    ];

    if let Ok(manager) = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT) {
        for name in &driver_names {
            if let Ok(driver_svc) = manager.open_service(
                *name,
                ServiceAccess::STOP | ServiceAccess::QUERY_STATUS,
            ) {
                if let Ok(status) = driver_svc.query_status() {
                    if status.current_state != ServiceState::Stopped {
                        info!(driver = %name, "Stopping WinDivert driver service via SCM");
                        let _ = driver_svc.stop();
                    }
                }
            }
        }
    }

    let net_exe = sys32.join("net.exe");
    if net_exe.is_file() {
        for name in &["windivert", "windivert14", "windivert22"] {
            let _ = std::process::Command::new(&net_exe)
                .args(["stop", name, "/y"])
                .output();
        }
    }
}

/// Forcibly terminates any running engine workers and unloads/deletes the WinDivert kernel driver.
/// This prevents error 1072 (ERROR_SERVICE_MARKED_FOR_DELETE) by ensuring all driver device handles
/// are released before driver service deletion.
pub fn cleanup_windivert_driver() -> Result<(), ServiceManagementError> {
    let sys_root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string());
    let sys32 = std::path::PathBuf::from(&sys_root).join("System32");

    // 1. Terminate all worker processes that might hold open handles to \\.\WinDivert
    let taskkill = sys32.join("taskkill.exe");
    if taskkill.is_file() {
        let worker_images = [
            "goodbyedpi.exe",
            "ciadpi.exe",
            "zondpi-engine-worker.exe",
            "zondpi-gui.exe",
            "zondpi-cli.exe",
            "zapret.exe",
            "byedpi.exe",
        ];
        for img in &worker_images {
            let _ = std::process::Command::new(&taskkill)
                .args(["/F", "/T", "/IM", img])
                .output();
        }
        // Give Windows kernel time to close handles and cleanup device references
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    // 2. Stop and delete candidate WinDivert driver services via Windows SCM API
    let driver_names = [
        "windivert",
        "windivert14",
        "windivert22",
        "WinDivert",
        "WinDivert14",
        "WinDivert22",
    ];

    if let Ok(manager) = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT) {
        for name in &driver_names {
            if let Ok(driver_svc) = manager.open_service(
                *name,
                ServiceAccess::STOP | ServiceAccess::DELETE | ServiceAccess::QUERY_STATUS,
            ) {
                if let Ok(status) = driver_svc.query_status() {
                    if status.current_state != ServiceState::Stopped {
                        info!(driver = %name, "Stopping WinDivert driver service via SCM");
                        let _ = driver_svc.stop();
                        std::thread::sleep(std::time::Duration::from_millis(300));
                    }
                }
                info!(driver = %name, "Deleting WinDivert driver service from SCM");
                let _ = driver_svc.delete();
            }
        }
    }

    // 3. Secondary hardened fallback via net.exe stop and sc.exe delete
    let net_exe = sys32.join("net.exe");
    let sc_exe = sys32.join("sc.exe");

    for name in &["windivert", "windivert14", "windivert22"] {
        if net_exe.is_file() {
            let _ = std::process::Command::new(&net_exe)
                .args(["stop", name, "/y"])
                .output();
        }
        if sc_exe.is_file() {
            let _ = std::process::Command::new(&sc_exe)
                .args(["delete", name])
                .output();
        }
    }

    // Secondary pass: trigger stop once more to immediately finalize any pending marked-for-deletion services
    for name in &["windivert", "windivert14", "windivert22"] {
        if net_exe.is_file() {
            let _ = std::process::Command::new(&net_exe)
                .args(["stop", name, "/y"])
                .output();
        }
    }

    std::thread::sleep(std::time::Duration::from_millis(300));
    info!("WinDivert kernel driver cleanup completed");
    Ok(())
}

/// Uninstalls ZonDPI service from Windows SCM and thoroughly purges WinDivert drivers.
pub fn uninstall_service() -> Result<(), ServiceManagementError> {
    // 1. Forcibly clean up WinDivert driver and worker processes first
    let _ = cleanup_windivert_driver();

    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;

    let service = match manager.open_service(
        SERVICE_NAME,
        ServiceAccess::STOP | ServiceAccess::DELETE | ServiceAccess::QUERY_STATUS,
    ) {
        Ok(s) => s,
        Err(windows_service::Error::Winapi(ref io_err)) if io_err.raw_os_error() == Some(1060) => {
            // ERROR_SERVICE_DOES_NOT_EXIST (1060 / 0x424) - Idempotent removal
            info!(
                "Service '{}' does not exist in SCM, continuing driver cleanup",
                SERVICE_NAME
            );
            let _ = cleanup_windivert_driver();
            if let Ok(paths) = crate::runtime_paths::RuntimePaths::discover() {
                let dns_ctrl = zondpi_dns::DnsCompatibilityController::new(&paths.data_root);
                let _ = dns_ctrl.restore_dns();
            }
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

    // 2. Final sweep of WinDivert driver services to ensure 100% clean teardown
    let _ = cleanup_windivert_driver();

    // 3. Ensure any active adapter DNS override is completely restored on uninstall
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
