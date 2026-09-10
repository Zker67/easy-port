//! Web 控制台的 Tauri command。
//!
//! 这些命令**只在桌面端可用**：Web 端不能改控制台自身的配置
//! （端口 / token / 自启），否则攻破一次即可把 token 改成攻击者的，永久驻留。

use tauri::State;

use super::auth;
use super::console::{WebConsoleView, WebStatus};
use super::server::{self, WebState};
use crate::commands::AppState;
use crate::tunnel::cloudflared;

type CmdResult<T> = Result<T, String>;

/// 当前配置与运行态。token 只回传「是否已设置」。
#[tauri::command]
pub async fn web_console_status(state: State<'_, AppState>) -> CmdResult<WebConsoleView> {
    Ok(state.console.view().await)
}

#[tauri::command]
pub async fn set_web_console_port(port: u16, state: State<'_, AppState>) -> CmdResult<()> {
    if port == 0 {
        return Err("端口需在 1–65535 之间".into());
    }
    // 与普通映射撞端口会让两者互相踩，开启前就拦下比开启后报错好
    if state.registry.find_by_port(port).await.is_some() {
        return Err(format!("端口 {port} 已被一条映射占用，请换一个"));
    }
    state.console.set_port(port).await?;
    persist(&state).await;
    Ok(())
}

#[tauri::command]
pub async fn set_web_console_label(
    label: Option<String>,
    state: State<'_, AppState>,
) -> CmdResult<()> {
    state.console.set_label(label).await;
    persist(&state).await;
    Ok(())
}

/// 生成新 token，**明文只在此刻返回一次**。
///
/// 之后只存哈希，无法再查看，只能重新生成。
/// 同时作废所有已有会话——否则旧设备仍持有有效 cookie，换 token 形同虚设。
#[tauri::command]
pub async fn regenerate_web_token(state: State<'_, AppState>) -> CmdResult<String> {
    let token = auth::generate_secret()?;
    // Argon2 是 CPU 密集的，别占住异步执行器线程
    let hash = {
        let t = token.clone();
        tokio::task::spawn_blocking(move || auth::hash_token(&t))
            .await
            .map_err(|e| format!("生成 token 失败：{e}"))??
    };
    state.console.set_token_hash(hash).await;
    persist(&state).await;
    Ok(token)
}

#[tauri::command]
pub async fn set_web_auto_start(enabled: bool, state: State<'_, AppState>) -> CmdResult<()> {
    state.console.set_auto_start(enabled).await;
    persist(&state).await;
    Ok(())
}

/// 开启控制台：起 HTTP 服务 → 建立隧道。
#[tauri::command]
pub async fn start_web_console(state: State<'_, AppState>) -> CmdResult<WebConsoleView> {
    if state.console.is_running().await {
        return Err("控制台已在运行".into());
    }
    // 没有 token 就开启，等于把控制面裸奔在公网上
    if !state.console.has_token().await {
        return Err("请先生成访问 token，再开启控制台".into());
    }

    let port = state.console.port().await;
    state.console.set_status(WebStatus::Starting).await;

    let web_state = WebState {
        console: state.console.clone(),
        registry: state.registry.clone(),
    };

    let shutdown = match server::serve(port, web_state).await {
        Ok(tx) => tx,
        Err(e) => {
            state.console.set_status(WebStatus::Failed(e.clone())).await;
            return Err(e);
        }
    };
    state.console.set_shutdown(shutdown).await;

    // 隧道建立失败时必须把刚起的服务停掉，
    // 否则会留下一个「监听着但外部访问不到」的半死状态
    match cloudflared::spawn(port).await {
        Ok(spawned) => {
            let pid = spawned.child.id();
            // 控制台的隧道不进 registry：那里的每个方法都是为「用户的映射」写的
            state.console.set_running(spawned.public_url, pid).await;
            super::watch_tunnel(state.console.clone(), spawned.child);
            Ok(state.console.view().await)
        }
        Err(e) => {
            let msg = e.to_string();
            super::shutdown_console(&state.console).await;
            state.console.set_status(WebStatus::Failed(msg.clone())).await;
            Err(msg)
        }
    }
}

/// 启动时按 auto_start 决定是否自动开启。
///
/// 由前端在确认引擎可用后调用一次，与 `restore_tunnels` 同一时机。
/// 返回是否真的开启了，失败只记录不抛错——控制台起不来不该阻断整个应用。
#[tauri::command]
pub async fn restore_web_console(state: State<'_, AppState>) -> CmdResult<bool> {
    if !state.console.auto_start().await || !state.console.has_token().await {
        return Ok(false);
    }
    match start_web_console(state).await {
        Ok(_) => Ok(true),
        Err(e) => {
            eprintln!("[easy-port] Web 控制台自动开启失败：{e}");
            Ok(false)
        }
    }
}

#[tauri::command]
pub async fn stop_web_console(state: State<'_, AppState>) -> CmdResult<()> {
    super::shutdown_console(&state.console).await;
    state.console.set_status(WebStatus::Stopped).await;
    Ok(())
}

/// 配置变更后立即落盘。
///
/// 与隧道配置共用一个 state.json，因此必须交给 registry 统一写入，
/// 否则两边各写各的会互相覆盖。
async fn persist(state: &State<'_, AppState>) {
    let snapshot = state.console.snapshot().await;
    state.registry.set_web_snapshot(snapshot).await;
}
