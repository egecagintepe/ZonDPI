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

/// Determines whether the specified driver service binary path belongs to ZonDPI.
fn is_zondpi_owned_driver_path(driver_path: &str) -> bool {
    let lower_path = driver_path.to_lowercase();
    // 1. Direct path check: contains zondpi
    if lower_path.contains("zondpi") {
        return true;
    }

    // 2. Relative to current executable's directory
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            let parent_str = parent.to_string_lossy().to_lowercase();
            if lower_path.contains(&parent_str) {
                return true;
            }
        }
    }

    // 3. Under standard ProgramFiles\ZonDPI directory
    if let Ok(prog_files) = std::env::var("ProgramFiles") {
        let expected = format!("{}\\zondpi", prog_files.to_lowercase());
        if lower_path.contains(&expected) {
            return true;
        }
    }

    false
}

/// Safely cleans up WinDivert driver services ONLY if owned by ZonDPI.
/// Handles ERROR_SERVICE_MARKED_FOR_DELETE (1072) with bounded wait and honest reporting.
/// NEVER executes global process termination (taskkill) or deletes external driver instances.
pub fn cleanup_owned_windivert_driver() -> Result<(), ServiceManagementError> {
    #[cfg(windows)]
    {
        use std::ptr::null;
        use tracing::warn;
        use windows_sys::Win32::Foundation::{
            GetLastError, ERROR_SERVICE_DOES_NOT_EXIST, ERROR_SERVICE_MARKED_FOR_DELETE,
        };
        use windows_sys::Win32::System::Services::{
            CloseServiceHandle, ControlService, DeleteService, OpenSCManagerW, OpenServiceW,
            QueryServiceConfigW, QueryServiceStatus, QUERY_SERVICE_CONFIGW, SC_MANAGER_CONNECT,
            SERVICE_CONTROL_STOP, SERVICE_QUERY_CONFIG, SERVICE_QUERY_STATUS, SERVICE_STATUS,
            SERVICE_STOP,
        };

        const DELETE: u32 = 0x00010000;

        let candidate_names = [
            "WinDivert",
            "WinDivert14",
            "WinDivert22",
            "windivert",
            "windivert14",
            "windivert22",
        ];

        unsafe {
            let scm = OpenSCManagerW(null(), null(), SC_MANAGER_CONNECT);
            if scm.is_null() {
                return Ok(());
            }

            for name in &candidate_names {
                let name_wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
                let svc = OpenServiceW(
                    scm,
                    name_wide.as_ptr(),
                    SERVICE_QUERY_CONFIG | SERVICE_QUERY_STATUS | SERVICE_STOP | DELETE,
                );

                if svc.is_null() {
                    continue;
                }

                // 1. Query service configuration to retrieve lpBinaryPathName and verify ownership
                let mut bytes_needed = 0u32;
                let _ = QueryServiceConfigW(svc, std::ptr::null_mut(), 0, &mut bytes_needed);
                let mut binary_path_opt: Option<String> = None;

                if bytes_needed > 0 {
                    let mut buf = vec![0u8; bytes_needed as usize];
                    let p_config = buf.as_mut_ptr() as *mut QUERY_SERVICE_CONFIGW;
                    if QueryServiceConfigW(svc, p_config, bytes_needed, &mut bytes_needed) != 0 {
                        let path_ptr = (*p_config).lpBinaryPathName;
                        if !path_ptr.is_null() {
                            let mut len = 0;
                            while *path_ptr.add(len) != 0 {
                                len += 1;
                            }
                            let slice = std::slice::from_raw_parts(path_ptr, len);
                            binary_path_opt = Some(String::from_utf16_lossy(slice));
                        }
                    }
                }

                let is_owned = match binary_path_opt {
                    Some(ref p) => is_zondpi_owned_driver_path(p),
                    None => false,
                };

                if !is_owned {
                    info!(
                        driver = %name,
                        path = ?binary_path_opt,
                        "WinDivert driver service is not owned by ZonDPI; preserving external driver"
                    );
                    CloseServiceHandle(svc);
                    continue;
                }

                info!(
                    driver = %name,
                    path = ?binary_path_opt,
                    "ZonDPI-owned WinDivert driver detected; proceeding with cleanup"
                );

                // 2. Stop the driver service if running
                let mut status: SERVICE_STATUS = std::mem::zeroed();
                if QueryServiceStatus(svc, &mut status) != 0
                    && status.dwCurrentState
                        != windows_sys::Win32::System::Services::SERVICE_STOPPED
                {
                    info!(driver = %name, "Stopping ZonDPI-owned WinDivert driver service");
                    let mut stop_status: SERVICE_STATUS = std::mem::zeroed();
                    let _ = ControlService(svc, SERVICE_CONTROL_STOP, &mut stop_status);
                    std::thread::sleep(std::time::Duration::from_millis(300));
                }

                // 3. Delete driver service and safely handle ERROR_SERVICE_MARKED_FOR_DELETE (1072)
                if DeleteService(svc) != 0 {
                    info!(driver = %name, "ZonDPI-owned WinDivert driver service deleted successfully");
                } else {
                    let err = GetLastError();
                    if err == ERROR_SERVICE_MARKED_FOR_DELETE {
                        info!(
                            driver = %name,
                            "Driver service is marked for deletion (1072); waiting bounded time for handle release"
                        );
                        let start = std::time::Instant::now();
                        let mut finalized = false;
                        while start.elapsed() < std::time::Duration::from_secs(2) {
                            std::thread::sleep(std::time::Duration::from_millis(250));
                            let mut st: SERVICE_STATUS = std::mem::zeroed();
                            if QueryServiceStatus(svc, &mut st) == 0
                                && GetLastError() == ERROR_SERVICE_DOES_NOT_EXIST
                            {
                                finalized = true;
                                break;
                            }
                        }

                        if finalized {
                            info!(driver = %name, "Driver service deletion finalized successfully");
                        } else {
                            info!(
                                driver = %name,
                                "WinDivert service marked for deletion (ERROR 1072); Windows kernel will finalize removal once remaining handles are closed or upon reboot"
                            );
                        }
                    } else if err == ERROR_SERVICE_DOES_NOT_EXIST {
                        // Already removed
                    } else {
                        warn!(driver = %name, error_code = err, "Driver service deletion returned SCM status");
                    }
                }

                CloseServiceHandle(svc);
            }

            CloseServiceHandle(scm);
        }
    }

    Ok(())
}

/// Uninstalls ZonDPI service from Windows SCM and cleans up ZonDPI-owned driver instances.
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
                "Service '{}' does not exist in SCM, verifying driver state",
                SERVICE_NAME
            );
            let _ = cleanup_owned_windivert_driver();
            if let Ok(paths) = crate::runtime_paths::RuntimePaths::discover() {
                let dns_ctrl = zondpi_dns::DnsCompatibilityController::new(&paths.data_root);
                let _ = dns_ctrl.restore_dns();
            }
            return Ok(());
        }
        Err(e) => return Err(e.into()),
    };

    // 1. Stop service first if running, allowing ZonDPI workers to exit and release device handles
    if let Ok(status) = service.query_status() {
        if status.current_state != ServiceState::Stopped {
            info!("Stopping ZonDPI service prior to deletion");
            let _ = service.stop();
            // Bounded wait for service to fully stop and worker handles to close
            let start = std::time::Instant::now();
            while start.elapsed() < std::time::Duration::from_secs(3) {
                std::thread::sleep(std::time::Duration::from_millis(200));
                if let Ok(st) = service.query_status() {
                    if st.current_state == ServiceState::Stopped {
                        break;
                    }
                }
            }
        }
    }

    // 2. Delete ZonDPI service from SCM
    info!("Deleting service '{}' from SCM", SERVICE_NAME);
    match service.delete() {
        Ok(_) => info!("Service '{}' deleted successfully", SERVICE_NAME),
        Err(windows_service::Error::Winapi(ref io_err)) if io_err.raw_os_error() == Some(1072) => {
            info!(
                "Service '{}' marked for deletion (1072); will finalize when handles close",
                SERVICE_NAME
            );
        }
        Err(e) => return Err(e.into()),
    }

    // 3. Ensure any active adapter DNS override is completely restored on uninstall
    if let Ok(paths) = crate::runtime_paths::RuntimePaths::discover() {
        let dns_ctrl = zondpi_dns::DnsCompatibilityController::new(&paths.data_root);
        let _ = dns_ctrl.restore_dns();
    }

    // 4. Clean up WinDivert driver ONLY if owned by ZonDPI
    let _ = cleanup_owned_windivert_driver();

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
