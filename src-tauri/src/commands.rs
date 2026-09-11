//! 暴露给前端的 Tauri command。
//!
//! 本层只依赖 `tunnel::provider` 与 `tunnel::registry` 的抽象（不变量 1）。

use std::net::{Ipv4Addr, SocketAddrV4, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tauri::State;

use crate::tunnel::cloudflared;
use crate::tunnel::provider::{EngineStatus, Tunnel, TunnelError, TunnelStatus};
use crate::tunnel::registry::{TunnelCounts, TunnelRegistry};

/// command 统一错误类型：只把面向用户的消息文本传给前端。
pub type CmdResult<T> = Result<T, String>;

/// 注入到 Tauri 的共享状态。registry 需要 `Arc` 以便监视任务持有弱引用之外的所有权。
pub struct AppState {
    pub registry: Arc<TunnelRegistry>,
    /// Web 远程控制台。独立于 registry：它暴露的是应用自己的控制面，
    /// 不是用户的服务，混进 registry 会让每个消费方都长出例外分支。
    pub console: crate::web::console::SharedConsole,
    /// 启动时读取配置产生的告警，供前端首次渲染时提示一次。
    pub startup_warning: Option<String>,
}

fn to_msg(e: TunnelError) -> String {
    e.to_string()
}

/// 检查本机 cloudflared 是否可用。
#[tauri::command]
pub async fn check_engine() -> EngineStatus {
    cloudflared::check_engine().await
}

/// 启动时的配置读取告警，无告警时返回 null。
#[tauri::command]
pub async fn startup_warning(state: State<'_, AppState>) -> CmdResult<Option<String>> {
    Ok(state.startup_warning.clone())
}

/// 建立隧道的最长等待秒数，供前端等待提示引用，避免两处硬编码漂移。
#[tauri::command]
pub async fn url_timeout_secs() -> u64 {
    cloudflared::URL_TIMEOUT_SECS
}

/// 探测本机端口是否有服务在监听。
///
/// 只做提示性校验：连不上时给出更准确的错误，避免用户拿到一条打不开的链接。
fn is_port_listening(port: u16) -> bool {
    let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port);
    TcpStream::connect_timeout(&addr.into(), Duration::from_millis(300)).is_ok()
}

/// 建立隧道的共用路径：校验 → spawn → 登记。
///
/// `existing_id` 为 `Some` 时复用既有条目（自动重连场景），避免同端口出现两行记录。
///
/// `pub(crate)`：Web 控制台的「开启已有映射」走同一条路径。
/// 刻意不给 Web 端另写一份——两条建立路径迟早会漂移，
/// 而漏掉的很可能正是 `is_port_listening` 这类校验。
pub(crate) async fn establish(
    registry: &Arc<TunnelRegistry>,
    port: u16,
    label: Option<String>,
    existing_id: Option<String>,
) -> Result<Tunnel, TunnelError> {
    if registry.is_port_mapped(port).await {
        return Err(TunnelError::PortAlreadyMapped(port));
    }
    if !is_port_listening(port) {
        return Err(TunnelError::PortNotListening(port));
    }

    let spawned = cloudflared::spawn(port).await?;

    let tunnel = Tunnel {
        id: uuid::Uuid::new_v4().to_string(),
        port,
        label,
        public_url: Some(spawned.public_url),
        status: TunnelStatus::Running,
        created_at: chrono::Utc::now().to_rfc3339(),
        expires_at: None,
        archived: false,
        favorite: false,
        site: None,
        // 复用已有条目时，insert 会用旧条目的标签覆盖这里的空值
        tags: Vec::new(),
    };

    Ok(registry
        .insert(tunnel, spawned.child, existing_id, spawned.metrics_port)
        .await)
}

/// 为指定本机端口创建一条公网映射。
///
/// `expire_minutes` 为 Some 时同时设定定时关闭，到期自动断开。
#[tauri::command]
pub async fn create_tunnel(
    port: u16,
    label: Option<String>,
    expire_minutes: Option<u32>,
    state: State<'_, AppState>,
) -> CmdResult<Tunnel> {
    // 该端口若已有历史记录（已断开），复用它而不是新增一行。
    let existing = state.registry.find_by_port(port).await;
    let mut tunnel = establish(&state.registry, port, label, existing)
        .await
        .map_err(to_msg)?;

    if let Some(minutes) = expire_minutes.filter(|m| *m > 0) {
        // 建立成功后再设定时；失败不回滚隧道，只是没有定时而已。
        match state.registry.set_expiry(&tunnel.id, Some(minutes)).await {
            Ok(deadline) => tunnel.expires_at = deadline,
            Err(e) => eprintln!("[easy-port] 设定定时关闭失败：{e}"),
        }
    }

    Ok(tunnel)
}

/// 设定或取消某条映射的定时关闭；`minutes` 为 None / 0 表示取消。
///
/// 返回到期时刻（RFC 3339），取消时为 null。
#[tauri::command]
pub async fn set_expiry(
    id: String,
    minutes: Option<u32>,
    state: State<'_, AppState>,
) -> CmdResult<Option<String>> {
    state
        .registry
        .set_expiry(&id, minutes)
        .await
        .map_err(to_msg)
}

/// 断开一条映射。
#[tauri::command]
pub async fn stop_tunnel(id: String, state: State<'_, AppState>) -> CmdResult<()> {
    state.registry.stop(&id).await.map_err(to_msg)
}

/// 从列表中移除一条映射记录。
#[tauri::command]
pub async fn remove_tunnel(id: String, state: State<'_, AppState>) -> CmdResult<()> {
    state.registry.remove(&id).await.map_err(to_msg)
}

/// 设置某条映射是否在下次启动时自动重建。
#[tauri::command]
pub async fn set_auto_start(
    id: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> CmdResult<()> {
    state
        .registry
        .set_auto_start(&id, enabled)
        .await
        .map_err(to_msg)
}

/// 单条恢复结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreOutcome {
    pub port: u16,
    pub ok: bool,
    /// 失败原因，成功时为 null。
    pub error: Option<String>,
}

/// 启动时重建所有标记了自动重连的隧道。
///
/// 注意：链接**不是**恢复的，而是重新分配的新链接（旧的随进程退出即失效）。
/// 由前端在确认引擎可用后触发一次，以便逐条回报失败原因。
#[tauri::command]
pub async fn restore_tunnels(state: State<'_, AppState>) -> CmdResult<Vec<RestoreOutcome>> {
    // 顺带校验落盘的站点标题与图标是否仍然准确。
    // 放在这里而不是 setup 钩子：此时确定处于 tokio 运行时内，且引擎已确认可用。
    // 它只读本机端口，与下面的重建互不依赖，失败也不影响重建结果。
    state.registry.spawn_site_refresh();

    let targets = state.registry.auto_start_targets().await;
    let mut outcomes = Vec::with_capacity(targets.len());

    // 串行重建：并发拉起多个 cloudflared 会同时争抢网络且难以归因失败。
    for (id, port, label) in targets {
        match establish(&state.registry, port, label, Some(id.clone())).await {
            Ok(_) => outcomes.push(RestoreOutcome {
                port,
                ok: true,
                error: None,
            }),
            Err(e) => {
                let msg = to_msg(e);
                // 失败的条目留在列表里并标注原因，而不是静默消失。
                state.registry.mark_failed(&id, msg.clone()).await;
                outcomes.push(RestoreOutcome {
                    port,
                    ok: false,
                    error: Some(msg),
                });
            }
        }
    }

    Ok(outcomes)
}

/// 列出全部映射。
#[tauri::command]
pub async fn list_tunnels(state: State<'_, AppState>) -> CmdResult<Vec<Tunnel>> {
    Ok(state.registry.list().await)
}

/// 各条映射的自动重连开关状态，键为隧道 id。
#[tauri::command]
pub async fn auto_start_flags(
    state: State<'_, AppState>,
) -> CmdResult<std::collections::HashMap<String, bool>> {
    Ok(state.registry.auto_start_flags().await)
}

/// 归档 / 取消归档单条记录。
///
/// 归档后从「映射」页隐藏，但仍保留在「历史」页；活跃映射不允许归档。
#[tauri::command]
pub async fn set_archived(
    id: String,
    archived: bool,
    state: State<'_, AppState>,
) -> CmdResult<()> {
    state
        .registry
        .set_archived(&id, archived)
        .await
        .map_err(to_msg)
}

/// 设置备注。传 `null` 或空串即清除备注。
///
/// 备注不再只属于创建那一刻：端口是长期存在的单位，
/// 「这个端口是干什么的」随时可以补充或修正。
#[tauri::command]
pub async fn set_label(
    id: String,
    label: Option<String>,
    state: State<'_, AppState>,
) -> CmdResult<()> {
    state.registry.set_label(&id, label).await.map_err(to_msg)
}

/// 覆盖式设置标签。空串与重复项由 registry 清理。
///
/// 标签与备注是两种东西：备注是每个端口一条的自由文本，
/// 标签是可跨端口复用的人工分类，用于筛选。
#[tauri::command]
pub async fn set_tags(
    id: String,
    tags: Vec<String>,
    state: State<'_, AppState>,
) -> CmdResult<()> {
    state.registry.set_tags(&id, tags).await.map_err(to_msg)
}

/// 当前用到的全部标签，供筛选栏与输入建议使用。
#[tauri::command]
pub async fn all_tags(state: State<'_, AppState>) -> CmdResult<Vec<String>> {
    Ok(state.registry.all_tags().await)
}

/// 各条活跃映射的访问统计，键为隧道 id。
///
/// 并发抓取而非逐条串行：条目多时串行会把轮询间隔拖爆。
/// 抓不到的条目直接缺席，前端按「无数据」处理——
/// 指标是附加信息，不该因为它失败就让整个列表报错。
#[tauri::command]
pub async fn tunnel_metrics(
    state: State<'_, AppState>,
) -> CmdResult<std::collections::HashMap<String, crate::tunnel::metrics::TunnelMetrics>> {
    let ports = state.registry.metrics_ports().await;

    // 用 tokio::spawn 起并发任务再逐个 await，避免为一个 join_all 引入 futures 依赖
    let tasks: Vec<_> = ports
        .into_iter()
        .map(|(id, port)| {
            tokio::spawn(async move {
                crate::tunnel::metrics::fetch(port).await.map(|m| (id, m))
            })
        })
        .collect();

    let mut out = std::collections::HashMap::new();
    for task in tasks {
        if let Ok(Some((id, m))) = task.await {
            out.insert(id, m);
        }
    }
    Ok(out)
}

/// 设置收藏状态。收藏的映射在「映射」页置顶成独立分区。
#[tauri::command]
pub async fn set_favorite(
    id: String,
    favorite: bool,
    state: State<'_, AppState>,
) -> CmdResult<()> {
    state
        .registry
        .set_favorite(&id, favorite)
        .await
        .map_err(to_msg)
}

/// 归档所有已断开 / 失败的记录，返回归档条数。
#[tauri::command]
pub async fn archive_inactive(state: State<'_, AppState>) -> CmdResult<usize> {
    Ok(state.registry.archive_inactive().await)
}

/// 彻底删除所有已归档的记录，返回删除条数。供「历史」页清空使用。
#[tauri::command]
pub async fn purge_archived(state: State<'_, AppState>) -> CmdResult<usize> {
    Ok(state.registry.purge_archived().await)
}

/// 当前活跃数量与历史累计数量。
#[tauri::command]
pub async fn tunnel_counts(state: State<'_, AppState>) -> CmdResult<TunnelCounts> {
    Ok(state.registry.counts().await)
}
