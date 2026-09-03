use std::time::Duration;
use zondpi_compatibility::{
    CompatibilityManager, DetectionStatus, MockSecurityDetector, SecurityEnvironment,
    SecurityProduct,
};
use zondpi_packet_engine::{EngineCapabilities, EngineId, HealthCheckResult};

fn healthy_result(engine: &str) -> HealthCheckResult {
    HealthCheckResult::healthy(Duration::from_millis(5), format!("{engine} verified OK"))
}

fn unhealthy_result(engine: &str, err: &str) -> HealthCheckResult {
    HealthCheckResult::unhealthy(format!("{engine} failed: {err}"))
}

#[test]
fn test_no_av_and_goodbye_healthy_recommends_goodbye() {
    let env = SecurityEnvironment::new(Vec::new(), DetectionStatus::Available);
    let detector = MockSecurityDetector::new(env);

    let goodbye_caps = EngineCapabilities::goodbye_dpi();
    let byedpi_caps = EngineCapabilities::bye_dpi();
    let goodbye_health = healthy_result("GoodbyeDPI");
    let byedpi_health = healthy_result("ByeDPI");

    let (rec, _detected_env) = CompatibilityManager::recommend_with_detector(
        &detector,
        &goodbye_caps,
        &byedpi_caps,
        &goodbye_health,
        &byedpi_health,
    );

    assert_eq!(rec.preferred_engine, Some(EngineId::GoodbyeDpi));
    assert_eq!(rec.fallback_engine, Some(EngineId::ByeDpi));
    assert!(rec.confidence >= 0.9);
}

#[test]
fn test_kaspersky_and_goodbye_healthy_recommends_goodbye() {
    // Crucial requirement: Kaspersky detected != automatic ByeDPI
    let kaspersky = SecurityProduct {
        display_name: "Kaspersky Plus".to_string(),
        product_state: 266240,
        path_to_signed_product_exe: Some("C:\\Program Files\\Kaspersky Lab\\avp.exe".to_string()),
        path_to_signed_reporting_exe: None,
    };
    let env = SecurityEnvironment::new(vec![kaspersky], DetectionStatus::Available);
    let detector = MockSecurityDetector::new(env);

    let goodbye_caps = EngineCapabilities::goodbye_dpi();
    let byedpi_caps = EngineCapabilities::bye_dpi();
    let goodbye_health = healthy_result("GoodbyeDPI");
    let byedpi_health = healthy_result("ByeDPI");

    let (rec, _detected_env) = CompatibilityManager::recommend_with_detector(
        &detector,
        &goodbye_caps,
        &byedpi_caps,
        &goodbye_health,
        &byedpi_health,
    );

    // GoodbyeDPI remains preferred because live health check passed
    assert_eq!(rec.preferred_engine, Some(EngineId::GoodbyeDpi));
    assert_eq!(rec.fallback_engine, Some(EngineId::ByeDpi));
    assert!(rec.reason.contains("Kaspersky is active"));
}

#[test]
fn test_kaspersky_and_goodbye_unhealthy_recommends_byedpi() {
    let kaspersky = SecurityProduct {
        display_name: "Kaspersky Premium".to_string(),
        product_state: 266240,
        path_to_signed_product_exe: Some("C:\\Program Files\\Kaspersky Lab\\avp.exe".to_string()),
        path_to_signed_reporting_exe: None,
    };
    let env = SecurityEnvironment::new(vec![kaspersky], DetectionStatus::Available);
    let detector = MockSecurityDetector::new(env);

    let goodbye_caps = EngineCapabilities::goodbye_dpi();
    let byedpi_caps = EngineCapabilities::bye_dpi();
    let goodbye_health = unhealthy_result("GoodbyeDPI", "WinDivert driver blocked / Access Denied");
    let byedpi_health = healthy_result("ByeDPI");

    let (rec, _detected_env) = CompatibilityManager::recommend_with_detector(
        &detector,
        &goodbye_caps,
        &byedpi_caps,
        &goodbye_health,
        &byedpi_health,
    );

    assert_eq!(rec.preferred_engine, Some(EngineId::ByeDpi));
    assert_eq!(rec.fallback_engine, None);
    assert!(rec.reason.contains("ByeDPI SOCKS5 engine"));
    assert_eq!(rec.confidence, 0.95);
}

#[test]
fn test_unknown_security_env_and_goodbye_healthy_recommends_goodbye() {
    let env = SecurityEnvironment::unavailable();
    let detector = MockSecurityDetector::new(env);

    let goodbye_caps = EngineCapabilities::goodbye_dpi();
    let byedpi_caps = EngineCapabilities::bye_dpi();
    let goodbye_health = healthy_result("GoodbyeDPI");
    let byedpi_health = healthy_result("ByeDPI");

    let (rec, detected_env) = CompatibilityManager::recommend_with_detector(
        &detector,
        &goodbye_caps,
        &byedpi_caps,
        &goodbye_health,
        &byedpi_health,
    );

    assert_eq!(detected_env.detection_status, DetectionStatus::Unavailable);
    assert_eq!(rec.preferred_engine, Some(EngineId::GoodbyeDpi));
}

#[test]
fn test_goodbye_unavailable_and_byedpi_healthy_recommends_byedpi() {
    let env = SecurityEnvironment::new(Vec::new(), DetectionStatus::Available);
    let detector = MockSecurityDetector::new(env);

    let goodbye_caps = EngineCapabilities::goodbye_dpi();
    let byedpi_caps = EngineCapabilities::bye_dpi();
    let goodbye_health = unhealthy_result("GoodbyeDPI", "goodbyedpi.exe not found on disk");
    let byedpi_health = healthy_result("ByeDPI");

    let (rec, _detected_env) = CompatibilityManager::recommend_with_detector(
        &detector,
        &goodbye_caps,
        &byedpi_caps,
        &goodbye_health,
        &byedpi_health,
    );

    assert_eq!(rec.preferred_engine, Some(EngineId::ByeDpi));
    assert_eq!(rec.fallback_engine, None);
}

#[test]
fn test_both_unhealthy_returns_actionable_error() {
    let env = SecurityEnvironment::new(Vec::new(), DetectionStatus::Available);
    let detector = MockSecurityDetector::new(env);

    let goodbye_caps = EngineCapabilities::goodbye_dpi();
    let byedpi_caps = EngineCapabilities::bye_dpi();
    let goodbye_health = unhealthy_result("GoodbyeDPI", "Driver signature revoked");
    let byedpi_health = unhealthy_result("ByeDPI", "Socket bind error port 1080");

    let (rec, _detected_env) = CompatibilityManager::recommend_with_detector(
        &detector,
        &goodbye_caps,
        &byedpi_caps,
        &goodbye_health,
        &byedpi_health,
    );

    assert_eq!(rec.preferred_engine, None);
    assert_eq!(rec.fallback_engine, None);
    assert_eq!(rec.confidence, 0.0);
    assert!(rec
        .reason
        .contains("Both engines failed health verification"));
    assert!(rec.reason.contains("Action required"));
}

#[test]
fn test_detector_failure_does_not_crash() {
    let env = SecurityEnvironment::failed("WMI Provider Error: Service Unavailable (0x80041002)");
    let detector = MockSecurityDetector::new(env);

    let goodbye_caps = EngineCapabilities::goodbye_dpi();
    let byedpi_caps = EngineCapabilities::bye_dpi();
    let goodbye_health = healthy_result("GoodbyeDPI");
    let byedpi_health = healthy_result("ByeDPI");

    // Must not panic or return error
    let (rec, detected_env) = CompatibilityManager::recommend_with_detector(
        &detector,
        &goodbye_caps,
        &byedpi_caps,
        &goodbye_health,
        &byedpi_health,
    );

    assert!(matches!(
        detected_env.detection_status,
        DetectionStatus::Failed(_)
    ));
    assert_eq!(rec.preferred_engine, Some(EngineId::GoodbyeDpi));
}
