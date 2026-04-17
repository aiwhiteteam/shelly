mod server;
mod hooks;
mod sessions;

use tauri::{Emitter, Manager};
use tauri::window::Color;

/// Bring the main window to the front on the active space (including fullscreen).
/// Safe to call from any thread — dispatches native calls to the main thread.
pub fn bring_to_front(handle: &tauri::AppHandle) {
    let h = handle.clone();
    let _ = handle.run_on_main_thread(move || {
        if let Some(window) = h.get_webview_window("main") {
            let _ = window.show();
            let _ = window.set_focus();
            #[cfg(target_os = "macos")]
            {
                use raw_window_handle::HasWindowHandle;
                if let Ok(wh) = window.window_handle() {
                    if let raw_window_handle::RawWindowHandle::AppKit(h) = wh.as_raw() {
                        unsafe {
                            use objc2::msg_send;
                            use objc2::runtime::AnyObject;
                            let ns_view = h.ns_view.as_ptr() as *mut AnyObject;
                            let ns_window: *mut AnyObject = msg_send![ns_view, window];
                            let _: () = msg_send![ns_window, orderFrontRegardless];
                        }
                    }
                }
            }
        }
    });
}

#[tauri::command]
fn get_sessions() -> serde_json::Value {
    let sessions = sessions::scan_all();
    serde_json::json!({
        "count": sessions.len(),
        "sessions": sessions
    })
}

#[tauri::command]
fn get_terminals() -> Vec<String> {
    sessions::detect_terminals()
}

#[tauri::command]
fn jump_to_terminal(terminal_app: String) {
    sessions::jump_to_terminal(&terminal_app, None);
}

#[tauri::command]
fn jump_to_session(session_id: String) {
    match sessions::find_terminal_for_session(&session_id) {
        Some((terminal, tty)) => {
            log::info!("Jumping to terminal '{}' (tty {:?}) for session {}", terminal, tty, session_id);
            sessions::jump_to_terminal(&terminal, tty.as_deref());
        }
        None => {
            log::warn!("No terminal found for session {}, falling back to first detected", session_id);
            let terminals = sessions::detect_terminals();
            if let Some(first) = terminals.first() {
                sessions::jump_to_terminal(first, None);
            }
        }
    }
}

#[tauri::command]
fn respond_permission(request_id: String, behavior: String) {
    server::resolve_permission(&request_id, &behavior);
}

#[tauri::command]
fn respond_question(request_id: String, permission_decision: String, updated_input: Option<serde_json::Value>) {
    server::resolve_pre_tool_use(&request_id, &permission_decision, updated_input);
}

#[tauri::command]
fn set_yolo_mode(enabled: bool) {
    server::set_yolo_mode(enabled);
}

#[tauri::command]
fn allow_tool_always(tool_name: String) {
    hooks::add_allow_rule(&tool_name);
    server::add_always_allow_tool(&tool_name);
}

#[tauri::command]
fn resize_window(window: tauri::WebviewWindow, height: f64) {
    let size = window.outer_size().unwrap_or(tauri::PhysicalSize { width: 520, height: 48 }.into());
    let scale = window.scale_factor().unwrap_or(1.0);
    let physical_height = (height * scale) as u32;
    let _ = window.set_size(tauri::PhysicalSize::new(size.width, physical_height));
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Clean up hooks on panic
    std::panic::set_hook(Box::new(|_| {
        hooks::uninstall();
    }));

    // Clean up hooks on Ctrl+C / SIGTERM
    let _ = ctrlc::set_handler(|| {
        hooks::uninstall();
        std::process::exit(0);
    });

    tauri::Builder::default()
        .setup(|app| {
            app.handle().plugin(
                tauri_plugin_log::Builder::default()
                    .level(log::LevelFilter::Info)
                    .build(),
            )?;

            // Make webview truly transparent on macOS and center horizontally at top
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_background_color(Some(Color(0, 0, 0, 0)));
                // Center horizontally while keeping y=0 (top of screen)
                if let Ok(monitor) = window.current_monitor() {
                    if let Some(monitor) = monitor {
                        let screen_width = monitor.size().width as f64 / monitor.scale_factor();
                        let window_width = 520.0;
                        let x = ((screen_width - window_width) / 2.0) as i32;
                        let _ = window.set_position(tauri::Position::Physical(
                            tauri::PhysicalPosition::new(
                                (x as f64 * monitor.scale_factor()) as i32,
                                0,
                            ),
                        ));
                    }
                }

                // Set window level and collection behavior to appear over fullscreen apps
                #[cfg(target_os = "macos")]
                {
                    use raw_window_handle::HasWindowHandle;
                    if let Ok(wh) = window.window_handle() {
                        if let raw_window_handle::RawWindowHandle::AppKit(handle) = wh.as_raw() {
                            unsafe {
                                use objc2::msg_send;
                                use objc2::runtime::AnyObject;
                                let ns_view = handle.ns_view.as_ptr() as *mut AnyObject;
                                let ns_window: *mut AnyObject = msg_send![ns_view, window];
                                // NSStatusWindowLevel (25) — above fullscreen windows
                                let _: () = msg_send![ns_window, setLevel: 25_i64];
                                // CanJoinAllSpaces (1) | FullScreenAuxiliary (256)
                                let _: () = msg_send![ns_window, setCollectionBehavior: 257_u64];
                            }
                        }
                    }
                }
            }


            // Register plugins
            app.handle().plugin(tauri_plugin_updater::Builder::new().build())?;
            app.handle().plugin(tauri_plugin_process::init())?;

            // Install hooks
            hooks::install();

            // Start HTTP server
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(server::start(handle));
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_sessions,
            get_terminals,
            jump_to_terminal,
            jump_to_session,
            respond_permission,
            respond_question,
            allow_tool_always,
            set_yolo_mode,
            resize_window,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            match event {
                tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit => {
                    hooks::uninstall();
                }
                // macOS: clicking dock icon re-shows and expands the island
                tauri::RunEvent::Reopen { .. } => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                    let _ = app.emit("shelly://reopen", ());
                }
                _ => {}
            }
        });
}
