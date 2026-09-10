//! 内嵌 HTTP 服务：路由、鉴权中间件与暴露面收敛。

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, Request, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tokio::sync::watch;

use super::assets;
use super::auth;
use super::console::SharedConsole;
use crate::tunnel::provider::Tunnel;
use crate::tunnel::registry::TunnelRegistry;

/// 会话 cookie 名。
const COOKIE_NAME: &str = "easyport_session";
/// 会话有效期（秒），与 auth::SESSION_TTL 对应。
const COOKIE_MAX_AGE: u64 = 12 * 60 * 60;
/// 登录请求体上限。43 字符的 token 加 JSON 外壳远不到 1 KB。
const LOGIN_BODY_LIMIT: usize = 1024;

#[derive(Clone)]
pub struct WebState {
    pub console: SharedConsole,
    pub registry: Arc<TunnelRegistry>,
}

#[derive(Deserialize)]
struct LoginBody {
    token: String,
}

/// Web 端可见的映射视图。
///
/// **刻意不直接序列化 `Tunnel`**：显式转换是一道闸，
/// 将来给 `Tunnel` 加敏感字段时，不会因为忘记收敛而自动外泄。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebTunnelView {
    id: String,
    port: u16,
    label: Option<String>,
    public_url: Option<String>,
    status: crate::tunnel::provider::TunnelStatus,
    site_title: Option<String>,
    tags: Vec<String>,
}

impl From<&Tunnel> for WebTunnelView {
    fn from(t: &Tunnel) -> Self {
        Self {
            id: t.id.clone(),
            port: t.port,
            label: t.label.clone(),
            public_url: t.public_url.clone(),
            status: t.status.clone(),
            // 只带标题不带图标：favicon 的 data URI 可能上百 KB，
            // 手机端列表不值得为它多传这些字节
            site_title: t.site.as_ref().and_then(|s| s.title.clone()),
            tags: t.tags.clone(),
        }
    }
}

/// 构建路由表。
///
/// 暴露面刻意收敛（见 02-security）：只读 + 开关**已有**映射。
/// 不提供新建、删除、改配置——尤其不能改控制台自身，
/// 否则攻破一次即可把 token 改成攻击者的，永久驻留。
pub fn build_router(state: WebState) -> Router {
    let protected = Router::new()
        .route("/api/tunnels", get(list_tunnels))
        .route("/api/tunnels/{id}/start", post(start_tunnel))
        .route("/api/tunnels/{id}/stop", post(stop_tunnel))
        .route("/api/logout", post(logout))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_session,
        ));

    Router::new()
        .route("/api/login", post(login))
        .merge(protected)
        // 静态资源来自 include_dir，不做文件系统路径拼接（杜绝路径穿越）
        .fallback(assets::serve)
        .layer(middleware::from_fn(security_headers))
        .with_state(state)
}

/// 统一的安全响应头。
async fn security_headers(req: Request, next: Next) -> Response {
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    // 全部资源同源，禁止内联脚本以外的一切外部加载
    h.insert(
        header::CONTENT_SECURITY_POLICY,
        "default-src 'self'; style-src 'self' 'unsafe-inline'; base-uri 'none'; form-action 'self'"
            .parse()
            .unwrap(),
    );
    h.insert(header::X_CONTENT_TYPE_OPTIONS, "nosniff".parse().unwrap());
    h.insert(header::REFERRER_POLICY, "no-referrer".parse().unwrap());
    h.insert(header::X_FRAME_OPTIONS, "DENY".parse().unwrap());
    res
}

/// 从请求头取来源标识，用于按来源限速。
///
/// 经 cloudflared 转发后，socket 地址恒为本机，真实来源在 `CF-Connecting-IP`。
/// **但该头可被伪造**（有人直接访问本机端口时），所以它只用于「按来源锁定」这一层，
/// 真正兜底的是 `AuthState` 里不依赖任何请求头的全局窗口计数。
fn source_of(headers: &HeaderMap) -> String {
    headers
        .get("cf-connecting-ip")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string()
}

async fn login(
    State(state): State<WebState>,
    headers: HeaderMap,
    body: Body,
) -> Response {
    let source = source_of(&headers);

    // 请求体上限：避免有人用超大 body 拖垮服务
    let bytes = match axum::body::to_bytes(body, LOGIN_BODY_LIMIT).await {
        Ok(b) => b,
        Err(_) => return unauthorized(),
    };
    let Ok(payload) = serde_json::from_slice::<LoginBody>(&bytes) else {
        return unauthorized();
    };

    {
        let mut a = state.console.auth.lock().await;
        if a.is_rate_limited(&source) {
            // 与「token 错误」返回完全相同的响应：区分了等于告诉攻击者
            // 「换个 IP 继续」或「这个前缀对了」
            return unauthorized();
        }
        a.record_attempt();
    }

    let Some(hash) = state.console.token_hash().await else {
        return unauthorized();
    };

    // Argon2 校验是 CPU 密集的（几十毫秒），放到阻塞线程池，
    // 否则会占住 async 执行器线程，几个并发请求就能拖慢整个服务
    let token = payload.token;
    let ok = tokio::task::spawn_blocking(move || auth::verify_token(&token, &hash))
        .await
        .unwrap_or(false);

    let mut a = state.console.auth.lock().await;
    if !ok {
        a.record_failure(&source);
        return unauthorized();
    }

    a.record_success(&source);
    let Ok(session) = a.create_session() else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    drop(a);

    // cookie 四个属性一个都不能少：
    // HttpOnly 挡 XSS 窃取、Secure 只走 HTTPS、SameSite=Strict 挡 CSRF
    let cookie = format!(
        "{COOKIE_NAME}={session}; HttpOnly; Secure; SameSite=Strict; Path=/; Max-Age={COOKIE_MAX_AGE}"
    );
    (
        StatusCode::OK,
        [(header::SET_COOKIE, cookie)],
        Json(serde_json::json!({ "ok": true })),
    )
        .into_response()
}

async fn logout(State(state): State<WebState>, headers: HeaderMap) -> Response {
    if let Some(id) = session_id(&headers) {
        state.console.auth.lock().await.revoke_session(&id);
    }
    let cleared =
        format!("{COOKIE_NAME}=; HttpOnly; Secure; SameSite=Strict; Path=/; Max-Age=0");
    (StatusCode::OK, [(header::SET_COOKIE, cleared)]).into_response()
}

/// 鉴权中间件：无有效会话一律 401。
async fn require_session(
    State(state): State<WebState>,
    req: Request,
    next: Next,
) -> Response {
    let Some(id) = session_id(req.headers()) else {
        return unauthorized();
    };
    if !state.console.auth.lock().await.is_valid_session(&id) {
        return unauthorized();
    }
    next.run(req).await
}

fn session_id(headers: &HeaderMap) -> Option<String> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;
    raw.split(';')
        .filter_map(|kv| kv.split_once('='))
        .find(|(k, _)| k.trim() == COOKIE_NAME)
        .map(|(_, v)| v.trim().to_string())
}

/// 统一的拒绝响应。文案固定，不透露任何区分信息。
fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({ "error": "认证失败" })),
    )
        .into_response()
}

async fn list_tunnels(State(state): State<WebState>) -> Response {
    let items: Vec<WebTunnelView> = state
        .registry
        .list()
        .await
        .iter()
        // 已归档的不给 Web 端：它们在桌面端已被主动收起
        .filter(|t| !t.archived)
        .map(WebTunnelView::from)
        .collect();
    Json(items).into_response()
}

/// 开启**已有**映射。
///
/// 只接受已存在条目的 id——Web 端不能凭端口号新建映射（02-security 的暴露面收敛）。
/// 走与桌面端相同的 `establish`，端口校验等前置条件一并复用。
async fn start_tunnel(State(state): State<WebState>, Path(id): Path<String>) -> Response {
    let Some((port, label)) = state.registry.find_port_by_id(&id).await else {
        return StatusCode::NOT_FOUND.into_response();
    };

    match crate::commands::establish(&state.registry, port, label, Some(id)).await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn stop_tunnel(State(state): State<WebState>, Path(id): Path<String>) -> Response {
    match state.registry.stop(&id).await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

/// 启动服务，返回关闭句柄。
///
/// 只监听 127.0.0.1：对外暴露完全交给 cloudflared，
/// 绑 0.0.0.0 会让同局域网的任何人都能直接摸到控制台。
pub async fn serve(port: u16, state: WebState) -> Result<watch::Sender<bool>, String> {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .map_err(|e| format!("无法监听本机 {port} 端口：{e}。换一个端口再试"))?;
    Ok(serve_on(listener, state))
}

/// 在已绑定的 listener 上起服务。
///
/// 供测试用端口 0 拿到系统分配的空闲端口——
/// 「先 bind 探测端口再 drop 重来」有竞态，换个进程可能抢先占用。
pub fn serve_on(listener: tokio::net::TcpListener, state: WebState) -> watch::Sender<bool> {
    let (tx, mut rx) = watch::channel(false);
    let router = build_router(state);

    tokio::spawn(async move {
        let shutdown = async move {
            // 收到 true 即停止接受新连接并退出
            while rx.changed().await.is_ok() {
                if *rx.borrow() {
                    break;
                }
            }
        };
        if let Err(e) = axum::serve(listener, router)
            .with_graceful_shutdown(shutdown)
            .await
        {
            eprintln!("[easy-port] Web 控制台服务退出：{e}");
        }
    });

    tx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 能从_cookie_头里取出会话_id() {
        let mut h = HeaderMap::new();
        h.insert(
            header::COOKIE,
            "other=1; easyport_session=abc123; more=2".parse().unwrap(),
        );
        assert_eq!(session_id(&h).as_deref(), Some("abc123"));

        let empty = HeaderMap::new();
        assert!(session_id(&empty).is_none());
    }

    #[test]
    fn 没有_cf_头时来源标识不为空() {
        // 直接访问本机端口时没有这个头，不能因此 panic 或产生空键
        let h = HeaderMap::new();
        assert_eq!(source_of(&h), "unknown");
    }

    #[test]
    fn web_视图不含站点图标() {
        use crate::tunnel::provider::{SiteInfo, TunnelStatus};
        let t = Tunnel {
            id: "x".into(),
            port: 3000,
            label: None,
            public_url: None,
            status: TunnelStatus::Stopped,
            created_at: "2026-01-01T00:00:00Z".into(),
            expires_at: None,
            archived: false,
            favorite: false,
            site: Some(SiteInfo {
                title: Some("标题".into()),
                icon: Some("data:image/png;base64,AAAA".into()),
            }),
            tags: vec![],
        };

        let json = serde_json::to_string(&WebTunnelView::from(&t)).unwrap();
        assert!(json.contains("标题"));
        // 图标可能上百 KB，手机端列表不传
        assert!(!json.contains("base64"), "不该把图标传给 Web 端：{json}");
    }
}
