//! 暴露给前端的 Tauri command。
//!
//! 本层只依赖 `tunnel::provider` 与 `tunnel::registry` 的抽象（不变量 1）。

use std::net::{Ipv4Addr, SocketAddrV4, TcpStream};
use std::time::Duration;

use tauri::State;

use crate::tunnel::provider::{EngineStatus, Tunnel, TunnelError, TunnelStatus};
use crate::tunnel::registry::{TunnelCounts, TunnelRegistry};
use crate::tunnel::cloudflared;

/// command 统一错误类型：只把面向用户的消息文本传给前端。
pub type CmdResult<T> = Result<T, String>;

fn to_msg(e: TunnelError) -> String {
    e.to_string()
}

/// 检查本机 cloudflared 是否可用。
#[tauri::command]
pub async fn check_engine() -> EngineStatus {
    cloudflared::check_engine().await
}

/// 探测本机端口是否有服务在监听。
///
/// 只做提示性校验：连不上时给出更准确的错误，避免用户拿到一条打不开的链接。
fn is_port_listening(port: u16) -> bool {
    let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port);
    TcpStream::connect_timeout(&addr.into(), Duration::from_millis(300)).is_ok()
}

/// 为指定本机端口创建一条公网映射。
#[tauri::command]
pub async fn create_tunnel(
    port: u16,
    label: Option<String>,
    registry: State<'_, TunnelRegistry>,
) -> CmdResult<Tunnel> {
    if registry.is_port_mapped(port).await {
        return Err(to_msg(TunnelError::PortAlreadyMapped(port)));
    }
    if !is_port_listening(port) {
        return Err(to_msg(TunnelError::PortNotListening(port)));
    }

    let spawned = cloudflared::spawn(port).await.map_err(to_msg)?;

    let tunnel = Tunnel {
        id: uuid::Uuid::new_v4().to_string(),
        port,
        label,
        public_url: Some(spawned.public_url),
        status: TunnelStatus::Running,
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    registry.insert(tunnel.clone(), spawned.child).await;
    Ok(tunnel)
}

/// 断开一条映射。
#[tauri::command]
pub async fn stop_tunnel(
    id: String,
    registry: State<'_, TunnelRegistry>,
) -> CmdResult<()> {
    registry.stop(&id).await.map_err(to_msg)
}

/// 从列表中移除一条已停止的映射。
#[tauri::command]
pub async fn remove_tunnel(
    id: String,
    registry: State<'_, TunnelRegistry>,
) -> CmdResult<()> {
    registry.remove(&id).await.map_err(to_msg)
}

/// 列出全部映射。
#[tauri::command]
pub async fn list_tunnels(registry: State<'_, TunnelRegistry>) -> CmdResult<Vec<Tunnel>> {
    Ok(registry.list().await)
}

/// 当前活跃数量与累计数量。
#[tauri::command]
pub async fn tunnel_counts(registry: State<'_, TunnelRegistry>) -> CmdResult<TunnelCounts> {
    Ok(registry.counts().await)
}
