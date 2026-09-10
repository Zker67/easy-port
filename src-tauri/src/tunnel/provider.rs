//! 穿透引擎抽象层。
//!
//! 见 AGENTS.md 不变量 1：cloudflared 是当前唯一实现，但上层（commands / UI）
//! 只依赖本模块的类型，不得直接引用具体 provider，以便后续接入 frp / ngrok。

use serde::{Deserialize, Serialize};

/// 隧道当前状态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "message")]
pub enum TunnelStatus {
    /// 子进程已拉起，尚未解析到公网链接。
    Starting,
    /// 已拿到公网链接，可对外访问。
    Running,
    /// 已被用户停止或进程自行退出。
    Stopped,
    /// 出错，附带面向用户的原因。
    Failed(String),
}

/// 一条隧道的完整对外快照，直接序列化给前端。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tunnel {
    pub id: String,
    /// 被映射的本机端口。
    pub port: u16,
    /// 用户备注，可为空。
    pub label: Option<String>,
    /// 公网链接，仅在 Running 状态下为 Some。
    pub public_url: Option<String>,
    pub status: TunnelStatus,
    /// 创建时间，RFC 3339。
    pub created_at: String,
}

/// provider 无关的错误类型。
#[derive(Debug, thiserror::Error)]
pub enum TunnelError {
    #[error("未找到 cloudflared，请先安装后重试")]
    EngineMissing,

    #[error("端口 {0} 上没有正在监听的服务")]
    PortNotListening(u16),

    #[error("端口 {0} 已经在映射中")]
    PortAlreadyMapped(u16),

    #[error("隧道不存在或已关闭")]
    TunnelNotFound,

    #[error("启动隧道失败：{0}")]
    SpawnFailed(String),

    #[error("等待公网链接超时，请检查网络连通性")]
    UrlTimeout,

    #[error("{0}")]
    Io(String),
}

impl From<std::io::Error> for TunnelError {
    fn from(e: std::io::Error) -> Self {
        TunnelError::Io(e.to_string())
    }
}

/// 引擎可用性检查结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineStatus {
    pub available: bool,
    /// 可用时给出版本号，便于排查。
    pub version: Option<String>,
    /// 引擎名，当前恒为 "cloudflared"。
    pub engine: String,
}
