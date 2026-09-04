//! Regression tests for Windows Service configuration constants and startup mode persistence.

use windows_service::service::ServiceStartType;
use zondpi_service::service_scm::{
    SERVICE_DESCRIPTION, SERVICE_DISPLAY_NAME, SERVICE_NAME, SERVICE_START_TYPE,
};

#[test]
fn test_regression_service_start_type_must_be_auto_start() {
    // Regression check: ZonDPI must be configured as AutoStart (SERVICE_AUTO_START = 2),
    // NEVER OnDemand / Manual (SERVICE_DEMAND_START = 3).
    assert_eq!(
        SERVICE_START_TYPE,
        ServiceStartType::AutoStart,
        "ZonDPI service must have ServiceStartType::AutoStart"
    );
    assert_eq!(
        SERVICE_START_TYPE.to_raw(),
        2,
        "Raw Windows SCM start type must be 2 (SERVICE_AUTO_START)"
    );
}

#[test]
fn test_regression_service_identity_constants() {
    assert_eq!(SERVICE_NAME, "ZonDPI");
    assert_eq!(SERVICE_DISPLAY_NAME, "ZonDPI Service");
    assert!(
        SERVICE_DESCRIPTION.len() >= 20,
        "Service description must be descriptive"
    );
}
