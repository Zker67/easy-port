//! Web 控制台的配置、运行态与生命周期。
//!
//! 特别端口刻意**不进 `TunnelRegistry`**：registry 的每个方法都是为
//! 「用户的映射」写的（计数、归档、标签、分区），把控制台混进去会让每个
//! 消费方都长出「除非它是特别的」判断，这种散落的例外分支正是 bug 的温床。

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::{watch, Mutex};

use super::auth::AuthState;

/// 默认监听端口。选一个不常用的，减少与用户既有服务撞车的概率。
pub const DEFAULT_PORT: u16 = 17650;

/// 控制台运行状态。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "message", rename_all = "camelCase")]
pub enum WebStatus {
    Stopped,
    Starting,
    Running,
    Failed(String),
}

/// 回传给前端的视图。
///
/// **刻意不含 token 哈希**：前端没有任何理由拿到它。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebConsoleView {
    pub port: u16,
    pub label: Option<String>,
    /// 是否已设置 token。开启前必须为 true。
    pub has_token: bool,
    pub status: WebStatus,
    /// 公网链接，仅运行时为 Some。**不落盘**（不变量 6）。
    pub public_url: Option<String>,
    pub auto_start: bool,
}

/// 可持久化部分。
///
/// `public_url` 与 `status` 是运行态，不落盘。
/// `token_hash` 落盘是必要的（否则每次重启都要重配），但**只存哈希不存明文**。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PersistedWebConsole {
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub token_hash: Option<String>,
    /// 开机自动开启。**默认关闭**：开着意味着开机即在公网挂一个控制入口。
    #[serde(default)]
    pub auto_start: bool,
}

/// 控制台的完整状态。
pub struct WebConsole {
    inner: Mutex<Inner>,
    /// 会话与限速。与配置分开加锁：HTTP 请求会高频读它，
    /// 不该和「改端口」这类低频操作抢同一把锁。
    pub auth: Mutex<AuthState>,
}

struct Inner {
    port: u16,
    label: Option<String>,
    token_hash: Option<String>,
    auto_start: bool,
    status: WebStatus,
    public_url: Option<String>,
    /// 触发 HTTP 服务优雅关闭。持有它即可从任意位置停服。
    shutdown: Option<watch::Sender<bool>>,
    /// 隧道子进程的 pid，用于关闭时终止。
    tunnel_pid: Option<u32>,
}

impl Default for Inner {
    fn default() -> Self {
        Self {
            port: DEFAULT_PORT,
            label: None,
            token_hash: None,
            auto_start: false,
            status: WebStatus::Stopped,
            public_url: None,
            shutdown: None,
            tunnel_pid: None,
        }
    }
}

impl WebConsole {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner::default()),
            auth: Mutex::new(AuthState::default()),
        }
    }

    /// 从落盘状态恢复配置（不自动开启，开启由上层按 auto_start 决定）。
    ///
    /// 直接构造而不走 `new()` + 加锁：`blocking_lock` 在 async 运行时里会 panic，
    /// 而这个函数会在 Tauri 的 setup 钩子里调用。
    pub fn from_persisted(p: &PersistedWebConsole) -> Self {
        Self {
            inner: Mutex::new(Inner {
                port: p.port.unwrap_or(DEFAULT_PORT),
                label: p.label.clone(),
                token_hash: p.token_hash.clone(),
                auto_start: p.auto_start,
                ..Inner::default()
            }),
            auth: Mutex::new(AuthState::default()),
        }
    }

    pub async fn snapshot(&self) -> PersistedWebConsole {
        let inner = self.inner.lock().await;
        PersistedWebConsole {
            port: Some(inner.port),
            label: inner.label.clone(),
            token_hash: inner.token_hash.clone(),
            auto_start: inner.auto_start,
        }
    }

    pub async fn view(&self) -> WebConsoleView {
        let inner = self.inner.lock().await;
        WebConsoleView {
            port: inner.port,
            label: inner.label.clone(),
            has_token: inner.token_hash.is_some(),
            status: inner.status.clone(),
            public_url: inner.public_url.clone(),
            auto_start: inner.auto_start,
        }
    }

    pub async fn port(&self) -> u16 {
        self.inner.lock().await.port
    }

    pub async fn is_running(&self) -> bool {
        matches!(
            self.inner.lock().await.status,
            WebStatus::Running | WebStatus::Starting
        )
    }

    pub async fn auto_start(&self) -> bool {
        self.inner.lock().await.auto_start
    }

    pub async fn has_token(&self) -> bool {
        self.inner.lock().await.token_hash.is_some()
    }

    /// 取 token 哈希用于校验。只在鉴权路径内部使用。
    pub async fn token_hash(&self) -> Option<String> {
        self.inner.lock().await.token_hash.clone()
    }

    /// 改监听端口。运行中不允许改——换端口意味着换 URL，
    /// 得先停再开，隐式重启会让用户措手不及。
    pub async fn set_port(&self, port: u16) -> Result<(), String> {
        let mut inner = self.inner.lock().await;
        if matches!(inner.status, WebStatus::Running | WebStatus::Starting) {
            return Err("请先关闭控制台再修改端口".into());
        }
        inner.port = port;
        Ok(())
    }

    pub async fn set_label(&self, label: Option<String>) {
        let mut inner = self.inner.lock().await;
        inner.label = label
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
    }

    pub async fn set_auto_start(&self, enabled: bool) {
        self.inner.lock().await.auto_start = enabled;
    }

    /// 写入新 token 的哈希，并作废所有已有会话。
    ///
    /// 作废会话是**必须**的：否则旧设备仍持有有效 cookie，换 token 形同虚设。
    pub async fn set_token_hash(&self, hash: String) {
        self.inner.lock().await.token_hash = Some(hash);
        self.auth.lock().await.revoke_all();
    }

    pub async fn set_status(&self, status: WebStatus) {
        let mut inner = self.inner.lock().await;
        // 离开运行态时清掉链接：留着会显示一条打不开的死链。
        if !matches!(status, WebStatus::Running) {
            inner.public_url = None;
        }
        inner.status = status;
    }

    pub async fn set_running(&self, public_url: String, pid: Option<u32>) {
        let mut inner = self.inner.lock().await;
        inner.status = WebStatus::Running;
        inner.public_url = Some(public_url);
        inner.tunnel_pid = pid;
    }

    pub async fn set_shutdown(&self, tx: watch::Sender<bool>) {
        self.inner.lock().await.shutdown = Some(tx);
    }

    /// 取出关闭句柄与隧道 pid，供调用方执行实际的停服与杀进程。
    ///
    /// 取出即置空：关闭是一次性动作，重复调用不该重复杀。
    pub async fn take_shutdown(&self) -> (Option<watch::Sender<bool>>, Option<u32>) {
        let mut inner = self.inner.lock().await;
        (inner.shutdown.take(), inner.tunnel_pid.take())
    }
}

impl Default for WebConsole {
    fn default() -> Self {
        Self::new()
    }
}

/// 共享句柄。
pub type SharedConsole = Arc<WebConsole>;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn 视图不泄漏_token_哈希() {
        let console = WebConsole::new();
        console.set_token_hash("$argon2id$v=19$fake".into()).await;

        let view = console.view().await;
        assert!(view.has_token, "应告知已设置");

        // 序列化后不能出现哈希本身
        let json = serde_json::to_string(&view).unwrap();
        assert!(!json.contains("argon2"), "视图不得携带哈希：{json}");
        assert!(!json.contains("tokenHash"));
    }

    #[tokio::test]
    async fn 运行中不允许改端口() {
        let console = WebConsole::new();
        assert!(console.set_port(18000).await.is_ok());
        assert_eq!(console.port().await, 18000);

        console.set_status(WebStatus::Running).await;
        assert!(
            console.set_port(19000).await.is_err(),
            "运行中改端口应被拒绝"
        );
        assert_eq!(console.port().await, 18000, "端口不应被改动");
    }

    #[tokio::test]
    async fn 重设_token_会作废所有会话() {
        let console = WebConsole::new();
        let session = console.auth.lock().await.create_session().unwrap();
        assert!(console.auth.lock().await.is_valid_session(&session));

        console.set_token_hash("$argon2id$new".into()).await;

        assert!(
            !console.auth.lock().await.is_valid_session(&session),
            "换 token 后旧会话必须失效"
        );
    }

    #[tokio::test]
    async fn 离开运行态会清掉公网链接() {
        let console = WebConsole::new();
        console.set_running("https://example.trycloudflare.com".into(), Some(1)).await;
        assert!(console.view().await.public_url.is_some());

        console.set_status(WebStatus::Stopped).await;
        assert!(
            console.view().await.public_url.is_none(),
            "停止后不该留下打不开的死链"
        );
    }

    #[tokio::test]
    async fn 落盘快照不含运行态且默认不自启() {
        let console = WebConsole::new();
        console.set_token_hash("$argon2id$v=19$x".into()).await;
        console.set_running("https://example.trycloudflare.com".into(), Some(1)).await;

        let snap = console.snapshot().await;
        assert!(!snap.auto_start, "auto_start 必须默认关闭");
        assert_eq!(snap.token_hash.as_deref(), Some("$argon2id$v=19$x"));

        // 快照序列化后不得出现公网链接（不变量 6）
        let json = serde_json::to_string(&snap).unwrap();
        assert!(!json.contains("trycloudflare"), "链接不得落盘：{json}");
    }

    #[test]
    fn 旧配置缺少_web_段时用默认值() {
        // serde(default) 的回归测试：旧 state.json 没有这一段也能读
        let p: PersistedWebConsole = serde_json::from_str("{}").unwrap();
        assert_eq!(p.port, None);
        assert!(p.token_hash.is_none());
        assert!(!p.auto_start);
    }
}
