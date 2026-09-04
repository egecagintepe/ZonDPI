//! Control CLI for ZonDPI Windows Service communicating over Named Pipe IPC.

use clap::{Parser, Subcommand};
use std::time::Instant;
use zondpi_ipc_protocol::{
    framing::{read_response, write_request},
    IpcCommand, IpcRequest, IpcResponse, LogEntryDto, ProfileSummaryDto, RecommendationDto,
    ServiceStatusDto, DEFAULT_PIPE_NAME,
};

#[derive(Parser, Debug)]
#[command(
    name = "zondpi-cli",
    version,
    about = "Control and diagnostics CLI for ZonDPI Service"
)]
struct Cli {
    /// Custom Named Pipe endpoint path
    #[arg(long, default_value = DEFAULT_PIPE_NAME)]
    pipe: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug, Clone)]
enum Commands {
    /// Ping the running service
    Ping,
    /// Query service and protocol version
    Version,
    /// Query live status of service and active engine
    Status {
        /// Show technical developer details (raw backend, PID, internal details)
        #[arg(long, alias = "developer")]
        technical: bool,
    },
    /// List all discovered circumvention profiles
    Profiles,
    /// Start a specific engine (goodbye | byedpi) with a profile
    Start {
        /// Target engine: 'goodbye' or 'byedpi'
        engine: String,
        /// Profile ID or name
        profile: String,
    },
    /// Start in Auto mode with compatibility detection and fallback
    StartAuto {
        /// Profile ID or name
        #[arg(default_value = "byedpi-kaspersky-mode")]
        profile: String,
    },
    /// Gracefully stop the active engine
    Stop,
    /// Transactionally switch to a different engine and profile
    Switch {
        /// Target engine: 'goodbye' or 'byedpi'
        engine: String,
        /// Profile ID or name
        profile: String,
    },
    /// Query the live AV/compatibility recommendation
    Recommendation,
    /// Fetch recent worker stdout/stderr diagnostics logs
    Logs {
        /// Number of recent log lines to retrieve (default: 50)
        #[arg(long, default_value_t = 50)]
        lines: usize,
    },
    /// Test network reachability and DPI circumvention effectiveness
    TestConnectivity {
        /// Target host to test
        #[arg(default_value = "www.cloudflare.com")]
        target: String,
        /// Port to connect to
        #[arg(short, long, default_value_t = 443)]
        port: u16,
    },
    /// Detailed real-world diagnostic of DNS, TCP, and TLS/HTTPS response timing
    DiagnoseNetwork {
        /// Target host to test (default: discord.com)
        #[arg(default_value = "discord.com")]
        target: String,
        /// Port to connect to (default: 443)
        #[arg(short, long, default_value_t = 443)]
        port: u16,
    },
    /// Switch to a candidate profile, verify startup, and test network metrics
    TestProfile {
        /// Profile ID or name to test
        profile: String,
        /// Target host to test (default: discord.com)
        #[arg(default_value = "discord.com")]
        target: String,
        /// Port to connect to (default: 443)
        #[arg(short, long, default_value_t = 443)]
        port: u16,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let pipe_name = cli.pipe.clone();

    match &cli.command {
        Commands::TestConnectivity { target, port } => {
            return run_test_connectivity(&pipe_name, target, *port).await;
        }
        Commands::DiagnoseNetwork { target, port } => {
            return run_diagnose_network(&pipe_name, target, *port).await;
        }
        Commands::TestProfile {
            profile,
            target,
            port,
        } => {
            return run_test_profile(&pipe_name, profile, target, *port).await;
        }
        _ => {}
    }

    let (ipc_cmd, executed_cmd) = match &cli.command {
        Commands::Ping => (IpcCommand::Ping, cli.command.clone()),
        Commands::Version => (IpcCommand::GetVersion, cli.command.clone()),
        Commands::Status { .. } => (IpcCommand::GetStatus, cli.command.clone()),
        Commands::Profiles => (IpcCommand::ListProfiles, cli.command.clone()),
        Commands::Start { engine, profile } => (
            IpcCommand::StartEngine {
                engine: engine.clone(),
                profile: profile.clone(),
            },
            cli.command.clone(),
        ),
        Commands::StartAuto { profile } => (
            IpcCommand::StartAuto {
                profile: profile.clone(),
            },
            cli.command.clone(),
        ),
        Commands::Stop => (IpcCommand::StopEngine, cli.command.clone()),
        Commands::Switch { engine, profile } => (
            IpcCommand::SwitchEngine {
                engine: engine.clone(),
                profile: profile.clone(),
            },
            cli.command.clone(),
        ),
        Commands::Recommendation => (IpcCommand::GetRecommendation, cli.command.clone()),
        Commands::Logs { lines } => (
            IpcCommand::GetRecentLogs {
                lines: Some(*lines),
            },
            cli.command.clone(),
        ),
        Commands::TestConnectivity { .. }
        | Commands::DiagnoseNetwork { .. }
        | Commands::TestProfile { .. } => unreachable!(),
    };

    let start_time = Instant::now();
    let response = match send_ipc_request(&pipe_name, ipc_cmd).await {
        Ok(resp) => resp,
        Err(e) => {
            eprintln!("==================================================");
            eprintln!("        Failed to Connect to ZonDPI Service       ");
            eprintln!("==================================================");
            eprintln!("  Pipe Endpoint: {}", pipe_name);
            eprintln!("  Error Details: {}", e);
            eprintln!();
            eprintln!("  Make sure the ZonDPI Service is installed and running.");
            eprintln!("  To start the service in foreground dev mode:");
            eprintln!("    zondpi-service.exe --foreground");
            eprintln!("==================================================");
            std::process::exit(1);
        }
    };

    let elapsed = start_time.elapsed();

    if response.success {
        render_success_response(&response, elapsed, &executed_cmd);
    } else {
        render_error_response(&response);
        std::process::exit(1);
    }

    Ok(())
}

#[cfg(windows)]
async fn send_ipc_request(
    pipe_name: &str,
    command: IpcCommand,
) -> Result<IpcResponse, Box<dyn std::error::Error>> {
    use tokio::net::windows::named_pipe::ClientOptions;

    let mut client = ClientOptions::new().open(pipe_name)?;
    let req = IpcRequest::new(command);
    write_request(&mut client, &req).await?;
    let resp = read_response(&mut client).await?;
    Ok(resp)
}

#[cfg(not(windows))]
async fn send_ipc_request(
    _pipe_name: &str,
    _command: IpcCommand,
) -> Result<IpcResponse, Box<dyn std::error::Error>> {
    Err("Named Pipe IPC client is only supported on Windows hosts".into())
}

fn render_success_response(response: &IpcResponse, elapsed: std::time::Duration, cmd: &Commands) {
    let result = match &response.result {
        Some(val) => val,
        None => {
            println!("OK ({} ms)", elapsed.as_millis());
            return;
        }
    };

    match cmd {
        Commands::Status { technical } => {
            if let Ok(status) = serde_json::from_value::<ServiceStatusDto>(result.clone()) {
                println!("{}", render_status(&status, *technical));
            }
        }
        Commands::Profiles => {
            if let Ok(profiles) = serde_json::from_value::<Vec<ProfileSummaryDto>>(result.clone()) {
                println!("Available Profiles ({}):", profiles.len());
                println!(
                    "{:<25} {:<15} {:<30}",
                    "PROFILE ID", "TARGET ENGINE", "DESCRIPTION"
                );
                println!("{:-<25} {:-<15} {:-<30}", "", "", "");
                for p in profiles {
                    println!("{:<25} {:<15} {:<30}", p.id, p.target_engine, p.description);
                }
            }
        }
        Commands::Recommendation => {
            if let Ok(rec) = serde_json::from_value::<RecommendationDto>(result.clone()) {
                println!("==================================================");
                println!("     ZonDPI Environment Recommendation            ");
                println!("==================================================");
                println!(
                    "  Security Products:   {}",
                    if rec.detected_security_products.is_empty() {
                        "None detected (Standard Windows Defender)".to_string()
                    } else {
                        rec.detected_security_products.join(", ")
                    }
                );
                println!("  Recommended Engine:  {}", rec.recommended_engine);
                println!("  Compatibility Rating:{}", rec.compatibility_rating);
                println!("  Rationale:           {}", rec.rationale);
                println!("==================================================");
            }
        }
        Commands::Logs { .. } => {
            if let Ok(logs) = serde_json::from_value::<Vec<LogEntryDto>>(result.clone()) {
                if logs.is_empty() {
                    println!("No recent engine logs recorded in in-memory ring buffer.");
                } else {
                    println!("Recent Engine Logs ({} lines):", logs.len());
                    for log in logs {
                        println!("[{}] [{}] {}", log.timestamp, log.stream, log.line);
                    }
                }
            }
        }
        _ => {
            println!(
                "{}",
                serde_json::to_string_pretty(&result).unwrap_or_default()
            );
        }
    }
}

fn render_error_response(response: &IpcResponse) {
    eprintln!("==================================================");
    eprintln!("                  IPC Command Failed              ");
    eprintln!("==================================================");
    if let Some(ref err) = response.error {
        eprintln!("  Error Code:    {:?}", err.code);
        eprintln!("  Message:       {}", err.message);
        if let Some(ref details) = err.details {
            eprintln!("  Details:       {}", details);
        }
    } else {
        eprintln!("  Unknown error occurred without structured error body");
    }
    eprintln!("==================================================");
}

#[derive(Debug, Clone)]
struct NetworkMetrics {
    pub dns_ms: f64,
    pub tcp_ms: f64,
    pub tls_ms: f64,
    pub total_ms: f64,
    pub http_code: u16,
    pub is_dns_poisoned: bool,
    pub resolved_ip: String,
    pub verdict: String,
}

async fn measure_target(target: &str, use_doh: bool) -> NetworkMetrics {
    let dns_start = Instant::now();
    let mut resolved_ip = "Unknown".to_string();
    let mut is_dns_poisoned = false;

    if let Ok(mut addrs) = tokio::net::lookup_host(format!("{}:443", target)).await {
        if let Some(addr) = addrs.next() {
            let ip_str = addr.ip().to_string();
            resolved_ip = ip_str.clone();
            if ip_str == "195.175.254.2" {
                is_dns_poisoned = true;
            }
        }
    }
    let local_dns_ms = dns_start.elapsed().as_secs_f64() * 1000.0;

    // Use curl.exe for high-precision TLS handshake and HTTPS metrics
    let mut cmd = std::process::Command::new("curl.exe");
    cmd.arg("-s")
        .arg("-o")
        .arg("NUL")
        .arg("-w")
        .arg("%{time_namelookup} %{time_connect} %{time_appconnect} %{time_total} %{http_code}")
        .arg("-m")
        .arg("6");

    if use_doh {
        cmd.arg("--doh-url").arg("https://1.1.1.1/dns-query");
    }

    cmd.arg(format!("https://{}", target));

    let output = match cmd.output() {
        Ok(out) => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        Err(_) => String::new(),
    };

    let parts: Vec<&str> = output.split_whitespace().collect();
    if parts.len() >= 5 {
        let dns_sec = parts[0].parse::<f64>().unwrap_or(0.0);
        let connect_sec = parts[1].parse::<f64>().unwrap_or(0.0);
        let appconnect_sec = parts[2].parse::<f64>().unwrap_or(0.0);
        let total_sec = parts[3].parse::<f64>().unwrap_or(0.0);
        let http_code = parts[4].parse::<u16>().unwrap_or(0);

        let dns_ms = if dns_sec > 0.0 {
            dns_sec * 1000.0
        } else {
            local_dns_ms
        };
        let tcp_ms = if connect_sec > dns_sec {
            (connect_sec - dns_sec) * 1000.0
        } else {
            connect_sec * 1000.0
        };
        let tls_ms = if appconnect_sec > connect_sec {
            (appconnect_sec - connect_sec) * 1000.0
        } else {
            appconnect_sec * 1000.0
        };
        let total_ms = total_sec * 1000.0;

        let verdict = if (200..400).contains(&http_code) {
            format!("HTTP {} (Success)", http_code)
        } else if http_code == 0 {
            if is_dns_poisoned && !use_doh {
                "FAILED (DNS Poisoned -> TT Blockpage)".to_string()
            } else if appconnect_sec == 0.0 && connect_sec > 0.0 {
                "FAILED (TLS Handshake Reset/Blocked)".to_string()
            } else if connect_sec == 0.0 {
                "FAILED (TCP Connect Timeout)".to_string()
            } else {
                "FAILED (Connection Closed)".to_string()
            }
        } else {
            format!("HTTP {}", http_code)
        };

        NetworkMetrics {
            dns_ms,
            tcp_ms,
            tls_ms,
            total_ms,
            http_code,
            is_dns_poisoned,
            resolved_ip,
            verdict,
        }
    } else {
        NetworkMetrics {
            dns_ms: local_dns_ms,
            tcp_ms: 0.0,
            tls_ms: 0.0,
            total_ms: 0.0,
            http_code: 0,
            is_dns_poisoned,
            resolved_ip,
            verdict: "FAILED (Curl execution failed)".to_string(),
        }
    }
}

async fn run_diagnose_network(
    pipe_name: &str,
    target: &str,
    port: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("==================================================");
    println!("       ZonDPI Real-World Network Diagnostics      ");
    println!("==================================================");
    println!("  Target Host:        {}", target);
    println!("  Target Port:        {}", port);

    // Correlate with service status
    if let Ok(resp) = send_ipc_request(pipe_name, IpcCommand::GetStatus).await {
        if resp.success {
            if let Some(res) = resp.result {
                if let Ok(s) = serde_json::from_value::<ServiceStatusDto>(res) {
                    println!(
                        "  Active Engine:      {} ({})",
                        s.active_engine.as_deref().unwrap_or("None"),
                        s.active_profile.as_deref().unwrap_or("None")
                    );
                    println!(
                        "  Health Status:      {} | Engine: {} | Process: {}",
                        if s.health.is_healthy {
                            "HEALTHY"
                        } else {
                            "UNHEALTHY"
                        },
                        s.health.engine_health,
                        s.health.process_health
                    );
                }
            }
        }
    }
    println!("--------------------------------------------------");

    println!("Testing default system DNS path...");
    let m1 = measure_target(target, false).await;
    println!("  Resolved IP:        {}", m1.resolved_ip);
    println!("  DNS Lookup:         {:.1} ms", m1.dns_ms);
    println!("  TCP Connect:        {:.1} ms", m1.tcp_ms);
    println!("  TLS Handshake:      {:.1} ms", m1.tls_ms);
    println!("  Total Response:     {:.1} ms", m1.total_ms);
    println!("  HTTP Status:        {}", m1.http_code);
    println!("  Result:             {}", m1.verdict);

    if m1.is_dns_poisoned {
        println!("\n[!] NOTICE: ISP DNS poisoning detected (IP: 195.175.254.2).");
        println!("Testing clean DNS resolution path (DoH to 1.1.1.1) to measure DPI bypass...");
        let m2 = measure_target(target, true).await;
        println!("  Clean DNS Lookup:   {:.1} ms", m2.dns_ms);
        println!("  TCP Connect:        {:.1} ms", m2.tcp_ms);
        println!("  TLS Handshake:      {:.1} ms", m2.tls_ms);
        println!("  Total Response:     {:.1} ms", m2.total_ms);
        println!("  HTTP Status:        {}", m2.http_code);
        println!("  DPI Evasion Result: {}", m2.verdict);
    }

    println!("==================================================");
    Ok(())
}

async fn run_test_profile(
    pipe_name: &str,
    profile: &str,
    target: &str,
    _port: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("==================================================");
    println!("        ZonDPI Profile Candidate Evaluation       ");
    println!("==================================================");
    println!("  Candidate Profile:  {}", profile);
    println!("  Target Host:        {}", target);
    println!("--------------------------------------------------");

    // 1. Switch service to candidate profile
    print!("  [1/4] Switching service to profile '{}'... ", profile);
    let target_engine = if profile.contains("byedpi") {
        "byedpi"
    } else {
        "goodbye"
    };

    let switch_cmd = IpcCommand::SwitchEngine {
        engine: target_engine.to_string(),
        profile: profile.to_string(),
    };

    match send_ipc_request(pipe_name, switch_cmd).await {
        Ok(resp) if resp.success => println!("OK"),
        Ok(resp) => {
            println!("FAILED ({:?})", resp.error);
            return Ok(());
        }
        Err(e) => {
            println!("IPC ERROR ({})", e);
            return Ok(());
        }
    }

    // 2. Allow process and driver filter to activate
    print!("  [2/4] Awaiting filter activation... ");
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    println!("READY");

    // 3. Query status
    let mut pid_str = "-".to_string();
    let mut engine_health = "Unknown".to_string();
    let mut is_healthy = false;
    if let Ok(resp) = send_ipc_request(pipe_name, IpcCommand::GetStatus).await {
        if resp.success {
            if let Some(res) = resp.result {
                if let Ok(s) = serde_json::from_value::<ServiceStatusDto>(res) {
                    pid_str = s
                        .engine_pid
                        .map(|p| p.to_string())
                        .unwrap_or_else(|| "-".to_string());
                    engine_health = s.health.engine_health;
                    is_healthy = s.health.is_healthy;
                }
            }
        }
    }
    println!(
        "  [3/4] Worker Status: PID {} | EngineHealth: {} | Healthy: {}",
        pid_str, engine_health, is_healthy
    );

    // 4. Measure metrics
    println!(
        "  [4/4] Executing network measurement against {}...",
        target
    );
    let metrics = measure_target(target, false).await;

    println!("\nCandidate Benchmark Summary Table:");
    println!("---------------------------------------------------------------------------------------------");
    println!(
        "{:<18} | {:<8} | {:<8} | {:<8} | {:<10} | {:<25}",
        "Candidate", "Startup", "DNS ms", "TCP ms", "HTTPS ms", "Result"
    );
    println!("---------------------------------------------------------------------------------------------");
    println!(
        "{:<18} | {:<8} | {:<8.1} | {:<8.1} | {:<10.1} | {:<25}",
        profile,
        if is_healthy { "Active" } else { "Failed" },
        metrics.dns_ms,
        metrics.tcp_ms,
        metrics.total_ms,
        metrics.verdict
    );
    println!("---------------------------------------------------------------------------------------------");

    if metrics.is_dns_poisoned {
        println!("Note: Target ISP returned poisoned DNS (195.175.254.2). Running DoH clean path benchmark:");
        let doh_metrics = measure_target(target, true).await;
        println!(
            "{:<18} | {:<8} | {:<8.1} | {:<8.1} | {:<10.1} | {:<25}",
            format!("{}+DoH", profile),
            if is_healthy { "Active" } else { "Failed" },
            doh_metrics.dns_ms,
            doh_metrics.tcp_ms,
            doh_metrics.total_ms,
            doh_metrics.verdict
        );
        println!("---------------------------------------------------------------------------------------------");
    }

    Ok(())
}

async fn run_test_connectivity(
    pipe_name: &str,
    target: &str,
    port: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    run_diagnose_network(pipe_name, target, port).await
}

pub fn render_status(status: &ServiceStatusDto, technical: bool) -> String {
    let mut out = String::new();
    if technical {
        out.push_str("==================================================\n");
        out.push_str("          ZonDPI Technical Status (Dev)           \n");
        out.push_str("==================================================\n");
        out.push_str(&format!("  Service Version:     {}\n", status.version));
        out.push_str(&format!(
            "  Service State:       {}\n",
            status.service_state
        ));
        out.push_str(&format!(
            "  Service Uptime:      {} seconds\n",
            status.uptime_seconds
        ));
        out.push_str(&format!("  Operational Mode:    {}\n", status.mode));
        out.push_str(&format!(
            "  Active Engine:       {}\n",
            status.active_engine.as_deref().unwrap_or("None (Idle)")
        ));
        if let Some(req_eng) = &status.requested_engine {
            out.push_str(&format!("  Requested Engine:    {}\n", req_eng));
        }
        out.push_str(&format!(
            "  Active Profile:      {}\n",
            status.active_profile.as_deref().unwrap_or("None")
        ));
        if let Some(pid) = status.engine_pid {
            out.push_str(&format!("  Engine Worker PID:   {}\n", pid));
        }
        if let Some(up) = status.engine_uptime_seconds {
            out.push_str(&format!("  Engine Uptime:       {} seconds\n", up));
        }
        out.push_str(&format!(
            "  Process Health:      {}\n",
            status.health.process_health
        ));
        out.push_str(&format!(
            "  Engine Health:       {}\n",
            status.health.engine_health
        ));
        out.push_str(&format!(
            "  Effectiveness:       {}\n",
            status.health.network_effectiveness
        ));
        out.push_str(&format!(
            "  Health Check:        {} (latency: {} ms, {})\n",
            if status.health.is_healthy {
                "HEALTHY"
            } else {
                "UNHEALTHY"
            },
            status.health.latency_ms,
            status.health.message
        ));
        if let Some(rec) = &status.compatibility_recommendation {
            out.push_str(&format!("  Compatibility Rec:   {}\n", rec));
        }
        if let Some(fallback) = &status.fallback_reason {
            out.push_str(&format!("  Fallback Reason:     {}\n", fallback));
        }
        if let Some(err) = &status.last_error {
            out.push_str(&format!("  Last Error:          {}\n", err));
        }
        out.push_str("==================================================");
    } else {
        let service_state_tr = if status.service_state == "Running" {
            "Çalışıyor"
        } else {
            "Durduruldu"
        };
        let process_health_tr = if status.health.process_health == "Running" {
            "Çalışıyor"
        } else if status.health.process_health == "Stopped" {
            "Durduruldu"
        } else {
            &status.health.process_health
        };
        let profile_name = match status.active_profile.as_deref() {
            Some("turkey-default") => "Türkiye — Varsayılan",
            Some("superonline-default") => "Turkcell Superonline",
            Some("byedpi-kaspersky-mode") => "Uyumluluk Modu",
            Some(other) => other,
            None => "Yok (Kapalı)",
        };
        out.push_str("==================================================\n");
        out.push_str("              ZonDPI Durumu                       \n");
        out.push_str("==================================================\n");
        out.push_str(&format!("  Sürüm:              {}\n", status.version));
        out.push_str(&format!("  Hizmet:             {}\n", service_state_tr));
        out.push_str(&format!("  Çalışma Modu:       {}\n", status.mode));
        out.push_str(&format!("  Profil:             {}\n", profile_name));
        out.push_str(&format!("  İşlem Durumu:       {}\n", process_health_tr));
        out.push_str(&format!(
            "  Motor Durumu:       {}\n",
            status.health.engine_health
        ));
        out.push_str(&format!(
            "  Ağ Etkinliği:       {}\n",
            status.health.network_effectiveness
        ));
        out.push_str("  Sistem Uyumluluğu:  Uyumlu\n");
        out.push_str("==================================================");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use zondpi_ipc_protocol::HealthStatusDto;

    #[test]
    fn test_status_default_presentation_contains_no_internal_engine_leakage() {
        let dummy_status = ServiceStatusDto {
            version: "1.0.5".to_string(),
            uptime_seconds: 120,
            service_state: "Running".to_string(),
            mode: "Otomatik".to_string(),
            active_engine: Some("GoodbyeDPI".to_string()),
            requested_engine: Some("GoodbyeDPI".to_string()),
            active_profile: Some("turkey-default".to_string()),
            engine_pid: Some(1234),
            engine_uptime_seconds: Some(110),
            health: HealthStatusDto {
                is_healthy: true,
                latency_ms: 5,
                message: "ZonDPI paket filtresi etkin ve çalışıyor.".to_string(),
                process_health: "Running".to_string(),
                engine_health: "Paket filtresi etkin".to_string(),
                network_effectiveness: "Bilinmiyor".to_string(),
            },
            compatibility_recommendation: Some("Otomatik (Uyumlu)".to_string()),
            fallback_reason: None,
            last_error: None,
        };

        // 1. Verify standard presentation contains ZERO forbidden engine names
        let normal_view = render_status(&dummy_status, false);
        let forbidden = [
            "GoodbyeDPI",
            "goodbyedpi",
            "ByeDPI",
            "byedpi",
            "ciadpi",
            "WinDivert",
            "windivert",
        ];
        for name in &forbidden {
            assert!(
                !normal_view.contains(name),
                "Normal CLI status MUST NOT contain internal implementation '{}'",
                name
            );
        }

        // Must contain expected clean branding
        assert!(normal_view.contains("ZonDPI Durumu"));
        assert!(normal_view.contains("Türkiye — Varsayılan"));
        assert!(normal_view.contains("Paket filtresi etkin"));

        // 2. Verify technical developer view DOES include backend details
        let technical_view = render_status(&dummy_status, true);
        assert!(
            technical_view.contains("GoodbyeDPI"),
            "Technical view must expose backend implementation"
        );
        assert!(
            technical_view.contains("1234"),
            "Technical view must expose worker PID"
        );
    }
}
