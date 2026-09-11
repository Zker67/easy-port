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
    ///
    /// **当前为预留状态，没有任何代码产生它**：`establish` 是拿到链接后才 insert，
    /// 插入时直接为 `Running`。要真正启用需把建立流程改为「先 insert 再异步等链接」，
    /// 属架构改动，见 plans/2026-09-10-ux-polish/09-improvements.md 的 A 条。
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
    /// 定时关闭的到期时刻，RFC 3339；未设定时为 None。
    ///
    /// 纯运行态、不落盘：隧道本身随进程退出即失效，
    /// 跨重启保留一个到期时间没有意义。
    pub expires_at: Option<String>,
    /// 是否已归档：归档后从「映射」页隐藏，但仍保留在「历史」页。
    ///
    /// 已断开的映射默认**不**归档，仍留在映射页方便一键重连。
    pub archived: bool,
    /// 是否收藏：收藏的映射在「映射」页置顶成独立分区。
    pub favorite: bool,
    /// 本机服务的站点信息（标题 / 图标），异步探测所得；未探测到为 None。
    ///
    /// **会落盘**，让列表启动即有名字可认；但每次启动都重新探测校验，
    /// 拿到不同结果就覆盖（见 `registry::spawn_site_refresh`）。
    /// 与 `public_url` 不同：旧链接是死链（有害），旧标题只是过时（仍可辨认）。
    pub site: Option<SiteInfo>,
    /// 人工分类标签，可有多个，跨端口复用。
    ///
    /// 与 `label`（备注）是两种东西：备注是每个端口一条的自由文本，
    /// 说明「这个端口是干什么的」；标签是可跨端口复用的分类，用来筛选。
    ///
    /// 随用随建：没有独立的标签管理，输入即创建；
    /// 某标签的最后一个端口移除它后，该标签自然从可选列表消失。
    #[serde(default)]
    pub tags: Vec<String>,
}

/// 被映射的本机服务的站点信息。
///
/// `PartialEq` 用于启动后重新探测时比对新旧结果，只在真正变化时才覆盖落盘值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteInfo {
    /// 网页标题（<title>），取不到时为 None。
    pub title: Option<String>,
    /// favicon 的 data URI，取不到时为 None。
    pub icon: Option<String>,
}

/// provider 无关的错误类型。
///
/// 文案约定：每条错误都要回答「我现在该做什么」，只陈述现象不给下一步的文案视为缺陷。
#[derive(Debug, thiserror::Error)]
pub enum TunnelError {
    #[error("未找到 cloudflared，请先安装后重试")]
    EngineMissing,

    // 探测只连 127.0.0.1，服务若仅绑定其他网卡会误判；文案需诚实说明探测范围，
    // 否则用户看着服务在跑却被告知「没有服务」，只会认为是工具坏了。
    #[error("端口 {0} 上没有检测到服务（仅探测 127.0.0.1）。请先启动本机服务；若服务已在运行，请确认它监听了 127.0.0.1")]
    PortNotListening(u16),

    #[error("端口 {0} 已在映射中，可在下方列表中找到它")]
    PortAlreadyMapped(u16),

    #[error("隧道不存在或已关闭，请刷新后重试")]
    TunnelNotFound,

    #[error("该映射未在运行，无法设定定时关闭")]
    TunnelNotActive,

    #[error("该映射正在运行，请先断开再归档")]
    TunnelStillActive,

    #[error("启动隧道失败：{0}")]
    SpawnFailed(String),

    // 秒数由 cloudflared::URL_TIMEOUT 传入，避免文案与实际超时漂移。
    #[error("{0} 秒内未能获取公网链接。请检查网络能否访问 Cloudflare，稍后重试")]
    UrlTimeout(u64),

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
    /// 可执行文件所在路径，供引擎页展示「从哪里找到的」。
    pub path: Option<String>,
    /// 本次运行是否在用随包释放出来的那一份。
    ///
    /// 注意与 `embedded` 的区别：本字段为 false 只说明「这次没用上」，
    /// 可能是构建时就没内嵌，也可能是内嵌了但释放失败回退到了 PATH。
    /// 两者对用户的含义完全不同，必须靠 `embedded` 才能区分。
    pub bundled: bool,
    /// 这个构建里到底有没有编进 cloudflared（cargo feature `embed-cloudflared`）。
    ///
    /// 有它才能说清「单文件自足」是否成立：
    /// `embedded && bundled` 才是真正的免安装单文件形态。
    pub embedded: bool,
    /// 内嵌副本的字节数，没有内嵌时为 None。用于在界面上说明体积从何而来。
    pub embedded_size: Option<u64>,
}
