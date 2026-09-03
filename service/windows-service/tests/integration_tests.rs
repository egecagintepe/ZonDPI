//! Real Windows backend integration and lifecycle tests.

use std::ffi::OsString;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::time::Duration;

use tokio::net::windows::named_pipe::ClientOptions;
use zondpi_ipc_protocol::{
    framing::{read_response, write_request},
    IpcCommand, IpcRequest,
};
use zondpi_packet_engine::{EngineBinaryResolver, EngineId};

/// Helper to resolve test base directory
fn get_workspace_root() -> PathBuf {
    let mut dir = std::env::current_dir().unwrap();
    while !dir.join("Cargo.toml").is_file() || !dir.join("artifacts").is_dir() {
        if let Some(parent) = dir.parent() {
            dir = parent.to_path_buf();
        } else {
            break;
        }
    }
    dir
}

#[tokio::test]
async fn test_real_byedpi_process_lifecycle_and_socks5_handshake() {
    let root = get_workspace_root();
    let byedpi_exe = EngineBinaryResolver::resolve(EngineId::ByeDpi, Some(&root));
    assert!(
        byedpi_exe.is_ok(),
        "ciadpi.exe should be resolved: {:?}",
        byedpi_exe
    );
    let exe_path = byedpi_exe.unwrap();
    assert!(exe_path.is_file());

    let test_port: u16 = 10888;
    let addr_str = format!("127.0.0.1:{}", test_port);

    // Ensure port is currently free
    let pre_bind = std::net::TcpListener::bind(&addr_str);
    assert!(
        pre_bind.is_ok(),
        "Port {} must be free before test",
        test_port
    );
    drop(pre_bind);

    // Configure Supervisor
    let args = vec![
        OsString::from("-i"),
        OsString::from("127.0.0.1"),
        OsString::from("-p"),
        OsString::from(test_port.to_string()),
        OsString::from("-s"),
        OsString::from("1"),
    ];

    let config =
        zondpi_service::supervisor::SupervisorConfig::new("Test-ByeDPI-Worker", exe_path, args);

    let supervisor = zondpi_service::supervisor::ProcessSupervisor::new(config);

    // 1. Spawn worker inside Job Object
    let spawn_res = supervisor.spawn().await;
    assert!(spawn_res.is_ok(), "Failed to spawn worker: {:?}", spawn_res);

    // Wait 400ms for socket initialization
    tokio::time::sleep(Duration::from_millis(400)).await;

    // 2. Verify state and PID
    let status = supervisor.status().await;
    assert_eq!(
        status.state,
        zondpi_service::supervisor::SupervisorState::Running
    );
    assert!(status.pid.is_some());
    assert!(supervisor.is_running().await);

    // 3. Connect to live SOCKS5 port and verify protocol handshake
    let target_addr: SocketAddr = addr_str.parse().unwrap();
    let stream_res = TcpStream::connect_timeout(&target_addr, Duration::from_millis(1000));
    assert!(
        stream_res.is_ok(),
        "Failed to connect to ByeDPI SOCKS5 port: {:?}",
        stream_res
    );
    let mut stream = stream_res.unwrap();

    let _ = stream.set_read_timeout(Some(Duration::from_millis(1000)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(1000)));

    // Send SOCKS5 greeting (VER=5, NMETHODS=1, METHOD=0 NO_AUTH)
    let greeting = [0x05, 0x01, 0x00];
    stream.write_all(&greeting).expect("write socks5 greeting");

    let mut response = [0u8; 2];
    stream
        .read_exact(&mut response)
        .expect("read socks5 response");

    // SOCKS5 response: [0x05, 0x00] -> Version 5, Method No Auth
    assert_eq!(response[0], 0x05, "SOCKS version must be 5");
    assert_eq!(
        response[1], 0x00,
        "SOCKS auth method must be 0x00 (NO AUTH)"
    );
    drop(stream);

    // 4. Test log buffer retrieval
    let logs = supervisor.get_recent_logs(Some(10)).await;
    assert!(logs.len() <= 200);

    // 5. Graceful shutdown
    let stop_res = supervisor.graceful_stop().await;
    assert!(stop_res.is_ok(), "Graceful stop failed: {:?}", stop_res);

    tokio::time::sleep(Duration::from_millis(300)).await;

    let post_status = supervisor.status().await;
    assert_eq!(
        post_status.state,
        zondpi_service::supervisor::SupervisorState::Stopped
    );
    assert_eq!(post_status.pid, None);
    assert!(!supervisor.is_running().await);

    // 6. Verify port is completely freed
    let post_bind = std::net::TcpListener::bind(&addr_str);
    assert!(
        post_bind.is_ok(),
        "Port must be freed after supervisor shutdown"
    );
}

#[tokio::test]
async fn test_named_pipe_ipc_end_to_end() {
    let paths = zondpi_service::runtime_paths::RuntimePaths::discover().unwrap();
    let mgr = zondpi_service::engine_mgr::EngineManager::new(paths);

    let test_pipe_name = r"\\.\pipe\zondpi-test-pipe-integration";
    let server = zondpi_service::ipc_server::IpcServer::new(test_pipe_name, mgr);

    let server_clone = server.clone();
    let server_task = tokio::spawn(async move {
        let _ = server_clone.run().await;
    });

    // Short wait for pipe creation
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Connect IPC client
    let mut client = ClientOptions::new()
        .open(test_pipe_name)
        .expect("open client pipe");

    // 1. Send Ping
    let ping_req = IpcRequest::new(IpcCommand::Ping);
    write_request(&mut client, &ping_req)
        .await
        .expect("write ping");
    let ping_resp = read_response(&mut client).await.expect("read ping resp");
    assert!(ping_resp.success);
    assert_eq!(ping_resp.request_id, ping_req.request_id);

    // 2. Send GetVersion
    let ver_req = IpcRequest::new(IpcCommand::GetVersion);
    write_request(&mut client, &ver_req)
        .await
        .expect("write ver");
    let ver_resp = read_response(&mut client).await.expect("read ver resp");
    assert!(ver_resp.success);
    assert!(ver_resp.result.is_some());

    // 3. Send GetStatus
    let st_req = IpcRequest::new(IpcCommand::GetStatus);
    write_request(&mut client, &st_req)
        .await
        .expect("write status");
    let st_resp = read_response(&mut client).await.expect("read status resp");
    assert!(st_resp.success);

    // 4. Send ListProfiles
    let prof_req = IpcRequest::new(IpcCommand::ListProfiles);
    write_request(&mut client, &prof_req)
        .await
        .expect("write list profiles");
    let prof_resp = read_response(&mut client)
        .await
        .expect("read list profiles resp");
    assert!(prof_resp.success);

    // Clean up
    drop(client);
    server.stop();
    server_task.abort();
}
