//! 活跃隧道注册表：进程句柄的唯一持有者，也是计数的唯一来源。
//!
//! 见 AGENTS.md 不变量 5：所有 spawn 必须经由本模块登记，
//! 保证应用退出时能遍历清理，不留孤儿 cloudflared 进程。

use std::collections::HashMap;

use serde::Serialize;
use tokio::process::Child;
use tokio::sync::Mutex;

use super::provider::{Tunnel, TunnelError, TunnelStatus};

/// 计数快照，前端只读展示，不在 UI 层维护第二份。
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelCounts {
    /// 当前活跃（Starting + Running）数量。
    pub active: u32,
    /// 本次应用启动以来累计创建成功的数量。
    pub total_created: u64,
}

struct Entry {
    tunnel: Tunnel,
    /// Stopped / Failed 的条目保留记录但不再持有进程。
    child: Option<Child>,
}

#[derive(Default)]
struct Inner {
    entries: HashMap<String, Entry>,
    total_created: u64,
}

#[derive(Default)]
pub struct TunnelRegistry {
    inner: Mutex<Inner>,
}

impl TunnelRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 端口是否已有活跃隧道。
    pub async fn is_port_mapped(&self, port: u16) -> bool {
        let inner = self.inner.lock().await;
        inner.entries.values().any(|e| {
            e.tunnel.port == port
                && matches!(
                    e.tunnel.status,
                    TunnelStatus::Starting | TunnelStatus::Running
                )
        })
    }

    /// 登记一条已成功建立的隧道。
    pub async fn insert(&self, tunnel: Tunnel, child: Child) {
        let mut inner = self.inner.lock().await;
        inner.total_created += 1;
        inner.entries.insert(
            tunnel.id.clone(),
            Entry {
                tunnel,
                child: Some(child),
            },
        );
    }

    /// 停止指定隧道并杀掉其子进程。
    pub async fn stop(&self, id: &str) -> Result<(), TunnelError> {
        let mut inner = self.inner.lock().await;
        let entry = inner
            .entries
            .get_mut(id)
            .ok_or(TunnelError::TunnelNotFound)?;

        if let Some(mut child) = entry.child.take() {
            let _ = child.kill().await;
        }
        entry.tunnel.status = TunnelStatus::Stopped;
        entry.tunnel.public_url = None;
        Ok(())
    }

    /// 移除一条记录（已停止的条目才允许移除）。
    pub async fn remove(&self, id: &str) -> Result<(), TunnelError> {
        let mut inner = self.inner.lock().await;
        if let Some(mut entry) = inner.entries.remove(id) {
            if let Some(mut child) = entry.child.take() {
                let _ = child.kill().await;
            }
            Ok(())
        } else {
            Err(TunnelError::TunnelNotFound)
        }
    }

    /// 列出全部隧道，按创建时间倒序。
    pub async fn list(&self) -> Vec<Tunnel> {
        let inner = self.inner.lock().await;
        let mut items: Vec<Tunnel> =
            inner.entries.values().map(|e| e.tunnel.clone()).collect();
        items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        items
    }

    /// 当前计数快照。
    pub async fn counts(&self) -> TunnelCounts {
        let inner = self.inner.lock().await;
        let active = inner
            .entries
            .values()
            .filter(|e| {
                matches!(
                    e.tunnel.status,
                    TunnelStatus::Starting | TunnelStatus::Running
                )
            })
            .count() as u32;
        TunnelCounts {
            active,
            total_created: inner.total_created,
        }
    }

    /// 应用退出时的兜底清理：杀掉所有仍存活的子进程。
    pub async fn shutdown_all(&self) {
        let mut inner = self.inner.lock().await;
        for entry in inner.entries.values_mut() {
            if let Some(mut child) = entry.child.take() {
                let _ = child.kill().await;
            }
            entry.tunnel.status = TunnelStatus::Stopped;
            entry.tunnel.public_url = None;
        }
    }
}
