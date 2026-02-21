#[cfg(target_os = "linux")]
mod ptt;

#[cfg(target_os = "windows")]
mod ptt_windows;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            start_ptt,
            stop_ptt,
            change_ptt_key,
        ])
        .setup(|app| {
            #[cfg(target_os = "linux")]
            {
                use tauri::Manager;
                let webview = app.get_webview_window("main").unwrap();
                webview
                    .with_webview(|wv| {
                        use webkit2gtk::{PermissionRequestExt, WebViewExt};
                        wv.inner().connect_permission_request(|_webview, request| {
                            request.allow();
                            true
                        });
                    })
                    .expect("Failed to set up webview permission handler");
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
async fn start_ptt(app: tauri::AppHandle, key: String) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        return ptt::start(app, &key).map_err(|e| e.to_string());
    }
    #[cfg(target_os = "windows")]
    {
        return ptt_windows::start(app, &key).map_err(|e| e.to_string());
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = (app, key);
        Err("Native PTT is only available on Linux and Windows".to_string())
    }
}

#[tauri::command]
async fn stop_ptt() -> Result<(), String> {
    #[cfg(target_os = "linux")]
    ptt::stop();
    #[cfg(target_os = "windows")]
    ptt_windows::stop();
    Ok(())
}

#[tauri::command]
async fn change_ptt_key(app: tauri::AppHandle, key: String) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        return ptt::change_key(app, &key).map_err(|e| e.to_string());
    }
    #[cfg(target_os = "windows")]
    {
        return ptt_windows::change_key(app, &key).map_err(|e| e.to_string());
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = (app, key);
        Ok(())
    }
}
