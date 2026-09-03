use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, State,
};
use tokio::net::windows::named_pipe::ClientOptions;
use zondpi_ipc_protocol::{
    framing::{read_response, write_request},
    IpcCommand, IpcRequest, IpcResponse, ServiceStatusDto, DEFAULT_PIPE_NAME,
};

struct IpcClient {
    pipe_name: String,
}

impl IpcClient {
    fn new() -> Self {
        Self {
            pipe_name: DEFAULT_PIPE_NAME.to_string(),
        }
    }

    async fn send_command(&self, command: IpcCommand) -> Result<IpcResponse, String> {
        let client = ClientOptions::new()
            .open(&self.pipe_name)
            .map_err(|e| format!("Failed to connect to backend: {}", e))?;

        let mut client = client;
        let request = IpcRequest::new(command);

        write_request(&mut client, &request)
            .await
            .map_err(|e| format!("IPC write error: {}", e))?;

        let response = read_response(&mut client)
            .await
            .map_err(|e| format!("IPC read error: {}", e))?;

        Ok(response)
    }
}

#[tauri::command]
async fn ipc_ping(client: State<'_, IpcClient>) -> Result<IpcResponse, String> {
    client.send_command(IpcCommand::Ping).await
}

#[tauri::command]
async fn ipc_status(client: State<'_, IpcClient>) -> Result<IpcResponse, String> {
    client.send_command(IpcCommand::GetStatus).await
}

#[tauri::command]
async fn ipc_start(
    client: State<'_, IpcClient>,
    engine: String,
    profile: String,
) -> Result<IpcResponse, String> {
    client
        .send_command(IpcCommand::StartEngine { engine, profile })
        .await
}

#[tauri::command]
async fn ipc_start_auto(
    client: State<'_, IpcClient>,
    profile: Option<String>,
) -> Result<IpcResponse, String> {
    let profile = profile.unwrap_or_else(|| "turkey-default".to_string());
    client.send_command(IpcCommand::StartAuto { profile }).await
}

#[tauri::command]
async fn ipc_profiles(client: State<'_, IpcClient>) -> Result<IpcResponse, String> {
    client.send_command(IpcCommand::ListProfiles).await
}

#[tauri::command]
async fn ipc_recommendation(client: State<'_, IpcClient>) -> Result<IpcResponse, String> {
    client.send_command(IpcCommand::GetRecommendation).await
}

#[tauri::command]
async fn ipc_logs(
    client: State<'_, IpcClient>,
    lines: Option<usize>,
) -> Result<IpcResponse, String> {
    client
        .send_command(IpcCommand::GetRecentLogs { lines })
        .await
}

#[tauri::command]
async fn ipc_stop(client: State<'_, IpcClient>) -> Result<IpcResponse, String> {
    client.send_command(IpcCommand::StopEngine).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(IpcClient::new())
        .setup(|app| {
            let title_item = MenuItem::with_id(app, "title", "ZonDPI", false, None::<&str>)?;
            let profile_item =
                MenuItem::with_id(app, "profile", "Türkiye — Varsayılan", false, None::<&str>)?;
            let sep1 = PredefinedMenuItem::separator(app)?;
            let show_item = MenuItem::with_id(app, "show", "Pencereyi Aç", true, None::<&str>)?;
            let toggle_item = MenuItem::with_id(
                app,
                "toggle",
                "Korumayı Başlat / Durdur",
                true,
                None::<&str>,
            )?;
            let sep2 = PredefinedMenuItem::separator(app)?;
            let quit_item = MenuItem::with_id(app, "quit", "ZonDPI'dan Çık", true, None::<&str>)?;
            let menu = Menu::with_items(
                app,
                &[
                    &title_item,
                    &profile_item,
                    &sep1,
                    &show_item,
                    &toggle_item,
                    &sep2,
                    &quit_item,
                ],
            )?;

            let mut tray_builder = TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("ZonDPI - Bağlantı Koruması")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "toggle" => {
                        let app_handle = app.clone();
                        tauri::async_runtime::spawn(async move {
                            let client = app_handle.state::<IpcClient>();
                            if let Ok(resp) = client.send_command(IpcCommand::GetStatus).await {
                                if let Some(res) = resp.result {
                                    if let Ok(s) = serde_json::from_value::<ServiceStatusDto>(res) {
                                        let is_running = s.service_state == "Running"
                                            || s.active_engine.is_some();
                                        if is_running {
                                            let _ =
                                                client.send_command(IpcCommand::StopEngine).await;
                                        } else {
                                            let _ = client
                                                .send_command(IpcCommand::StartAuto {
                                                    profile: "turkey-default".to_string(),
                                                })
                                                .await;
                                        }
                                    }
                                }
                            }
                        });
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                });

            if let Some(icon) = app.default_window_icon() {
                tray_builder = tray_builder.icon(icon.clone());
            }

            let _tray = tray_builder.build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            ipc_ping,
            ipc_status,
            ipc_start,
            ipc_start_auto,
            ipc_stop,
            ipc_profiles,
            ipc_recommendation,
            ipc_logs
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
