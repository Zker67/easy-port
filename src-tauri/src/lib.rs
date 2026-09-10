mod commands;
pub mod tunnel;

use tauri::{Manager, RunEvent};

use tunnel::registry::TunnelRegistry;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(TunnelRegistry::new())
        .invoke_handler(tauri::generate_handler![
            commands::check_engine,
            commands::create_tunnel,
            commands::stop_tunnel,
            commands::remove_tunnel,
            commands::list_tunnels,
            commands::tunnel_counts,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|handle, event| {
        // 不变量 5：应用退出前必须回收所有 cloudflared 子进程。
        if let RunEvent::Exit = event {
            let registry = handle.state::<TunnelRegistry>();
            tauri::async_runtime::block_on(registry.shutdown_all());
        }
    });
}
