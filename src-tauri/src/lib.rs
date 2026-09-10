mod commands;
pub mod store;
pub mod tunnel;
pub mod web;

use std::sync::Arc;

use tauri::{Manager, RunEvent};

use commands::AppState;
use store::StateStore;
use tunnel::registry::TunnelRegistry;

/// 解析可用的 cloudflared 路径。
///
/// 顺序：内嵌释放（发布形态）→ 主程序同级 sidecar → None（由实现回退到 PATH）。
fn resolve_cloudflared(app: &tauri::App) -> Option<std::path::PathBuf> {
    // 1. 内嵌：释放到 app data 目录。失败不阻断启动，继续往下找。
    #[cfg(feature = "embed-cloudflared")]
    if let Ok(dir) = app.path().app_data_dir() {
        match tunnel::cloudflared::extract_embedded(&dir.join("engine")) {
            Ok(path) => return Some(path),
            Err(e) => eprintln!("[easy-port] 释放内置 cloudflared 失败：{e}"),
        }
    }

    // 2. 主程序同级（externalBin 形态或用户手动放置）
    let beside = std::env::current_exe().ok().and_then(|exe| {
        exe.parent().map(|dir| {
            dir.join(if cfg!(windows) {
                "cloudflared.exe"
            } else {
                "cloudflared"
            })
        })
    });
    if beside.as_ref().is_some_and(|p| p.is_file()) {
        return beside;
    }

    // 3. 交给 set_sidecar 登记 None，运行时回退到 PATH
    let _ = app;
    None
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            // 开机自启由官方插件管理（Windows 走注册表 Run 项），不自己写注册表。
            #[cfg(desktop)]
            app.handle().plugin(tauri_plugin_autostart::init(
                tauri_plugin_autostart::MacosLauncher::LaunchAgent,
                None,
            ))?;

            // cloudflared 解析顺序：内嵌释放 → 主程序同级 → PATH。
            // 内嵌是发布形态（单 exe 一体化）；后两者供开发期与特殊部署使用。
            let resolved = resolve_cloudflared(app);
            tunnel::cloudflared::set_sidecar(resolved);

            // app data 目录取不到时降级为内存模式，不阻断启动。
            let store = match app.path().app_data_dir() {
                Ok(dir) => StateStore::in_dir(dir),
                Err(e) => {
                    eprintln!("[easy-port] 无法定位配置目录：{e}");
                    StateStore::new(None)
                }
            };

            let (registry, startup_warning) = TunnelRegistry::with_store(store);
            // 控制台的配置与隧道配置同在一个 state.json，从 registry 读回
            let console = {
                let persisted = registry.web_snapshot_blocking();
                Arc::new(web::console::WebConsole::from_persisted(&persisted))
            };
            app.manage(AppState {
                registry: Arc::new(registry),
                console,
                startup_warning,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::check_engine,
            commands::startup_warning,
            commands::url_timeout_secs,
            commands::create_tunnel,
            commands::stop_tunnel,
            commands::remove_tunnel,
            commands::set_auto_start,
            commands::set_expiry,
            commands::restore_tunnels,
            commands::list_tunnels,
            commands::auto_start_flags,
            commands::set_label,
            commands::set_tags,
            commands::all_tags,
            commands::set_favorite,
            commands::set_archived,
            commands::archive_inactive,
            commands::purge_archived,
            commands::tunnel_counts,
            web::commands::web_console_status,
            web::commands::set_web_console_port,
            web::commands::set_web_console_label,
            web::commands::regenerate_web_token,
            web::commands::set_web_auto_start,
            web::commands::start_web_console,
            web::commands::restore_web_console,
            web::commands::stop_web_console,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|handle, event| {
        // 不变量 5：应用退出前必须回收所有 cloudflared 子进程。
        if let RunEvent::Exit = event {
            let state = handle.state::<AppState>();
            let registry = Arc::clone(&state.registry);
            let console = Arc::clone(&state.console);
            tauri::async_runtime::block_on(async move {
                registry.shutdown_all().await;
                // 控制台的隧道不在 registry 里，必须单独收——
                // 漏掉这一句会在退出后留下孤儿 cloudflared 进程
                web::shutdown_console(&console).await;
            });
        }
    });
}
