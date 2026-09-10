//! Web 控制台的集成测试：真的起一个 HTTP 服务，用真实请求验证鉴权。
//!
//! 这些是 02-security 里安全措施的回归测试。**删改任何一条前先看那份文档**：
//! 它们不是「覆盖率」，而是「这个功能凭什么敢挂到公网上」的证据。

use std::sync::Arc;

use easy_port_lib::store::StateStore;
use easy_port_lib::tunnel::registry::TunnelRegistry;
use easy_port_lib::web::auth;
use easy_port_lib::web::console::WebConsole;
use easy_port_lib::web::server::{self, WebState};

/// 起一个监听随机端口的控制台，返回 (基址, token, console)。
async fn spawn_server() -> (String, String, Arc<WebConsole>) {
    let console = Arc::new(WebConsole::new());
    let token = auth::generate_secret().unwrap();
    console.set_token_hash(auth::hash_token(&token).unwrap()).await;

    let registry = Arc::new(TunnelRegistry::with_store(StateStore::new(None)).0);
    let state = WebState {
        console: Arc::clone(&console),
        registry,
    };

    // 端口 0 让系统分配空闲端口。直接把 listener 交给服务，
    // 不走「bind 探测再 drop 重来」——那中间有竞态窗口
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let shutdown = server::serve_on(listener, state);
    console.set_shutdown(shutdown).await;

    (format!("http://127.0.0.1:{port}"), token, console)
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        // 不自动跟随重定向，也不自动存 cookie：cookie 由测试显式控制
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap()
}

async fn login(base: &str, token: &str) -> reqwest::Response {
    client()
        .post(format!("{base}/api/login"))
        .json(&serde_json::json!({ "token": token }))
        .send()
        .await
        .unwrap()
}

#[tokio::test]
async fn 未登录访问受保护接口一律_401() {
    let (base, _token, _c) = spawn_server().await;

    for path in ["/api/tunnels", "/api/logout"] {
        let res = client()
            .get(format!("{base}{path}"))
            .send()
            .await
            .unwrap();
        assert_eq!(
            res.status(),
            401,
            "{path} 在未登录时必须拒绝，实际 {}",
            res.status()
        );
    }
}

#[tokio::test]
async fn 正确_token_能登录且_cookie_属性齐全() {
    let (base, token, _c) = spawn_server().await;

    let res = login(&base, &token).await;
    assert_eq!(res.status(), 200);

    let cookie = res
        .headers()
        .get("set-cookie")
        .expect("必须下发会话 cookie")
        .to_str()
        .unwrap()
        .to_string();

    // 四个属性一个都不能少，理由见 02-security
    assert!(cookie.contains("HttpOnly"), "缺 HttpOnly：{cookie}");
    assert!(cookie.contains("Secure"), "缺 Secure：{cookie}");
    assert!(cookie.contains("SameSite=Strict"), "缺 SameSite：{cookie}");
    assert!(cookie.contains("Max-Age="), "缺 Max-Age：{cookie}");

    // 带上 cookie 后能访问受保护接口
    let session = cookie.split(';').next().unwrap();
    let res = client()
        .get(format!("{base}/api/tunnels"))
        .header("cookie", session)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
}

#[tokio::test]
async fn 错误_token_被拒且响应与限速不可区分() {
    let (base, _token, _c) = spawn_server().await;

    let wrong = login(&base, "definitely-not-the-token").await;
    assert_eq!(wrong.status(), 401);
    assert!(
        wrong.headers().get("set-cookie").is_none(),
        "失败时不该下发 cookie"
    );
    let wrong_body = wrong.text().await.unwrap();

    // 连续失败触发锁定后，响应必须和「token 错误」完全一致——
    // 区分了等于告诉攻击者「换个 IP 继续」
    for _ in 0..6 {
        let _ = login(&base, "still-wrong").await;
    }
    let limited = login(&base, "still-wrong").await;
    assert_eq!(limited.status(), 401);
    assert_eq!(
        limited.text().await.unwrap(),
        wrong_body,
        "限速与认证失败的响应必须不可区分"
    );
}

#[tokio::test]
async fn 限速期间即使_token_正确也不放行() {
    let (base, token, _c) = spawn_server().await;

    // 先把这个来源打到锁定
    for _ in 0..6 {
        let _ = login(&base, "wrong").await;
    }

    let res = login(&base, &token).await;
    assert_eq!(
        res.status(),
        401,
        "锁定期内正确 token 也应被拒，否则限速形同虚设"
    );
}

#[tokio::test]
async fn 重新生成_token_后旧会话立即失效() {
    let (base, token, console) = spawn_server().await;

    let res = login(&base, &token).await;
    let cookie = res.headers().get("set-cookie").unwrap().to_str().unwrap();
    let session = cookie.split(';').next().unwrap().to_string();

    // 确认此刻可用
    let ok = client()
        .get(format!("{base}/api/tunnels"))
        .header("cookie", &session)
        .send()
        .await
        .unwrap();
    assert_eq!(ok.status(), 200);

    // 换 token
    let new_token = auth::generate_secret().unwrap();
    console
        .set_token_hash(auth::hash_token(&new_token).unwrap())
        .await;

    let after = client()
        .get(format!("{base}/api/tunnels"))
        .header("cookie", &session)
        .send()
        .await
        .unwrap();
    assert_eq!(
        after.status(),
        401,
        "换 token 后旧设备必须被踢下线，否则换 token 没有意义"
    );
}

#[tokio::test]
async fn 安全响应头齐全() {
    let (base, _token, _c) = spawn_server().await;

    let res = client().get(format!("{base}/")).send().await.unwrap();
    let h = res.headers();

    assert!(h.get("content-security-policy").is_some(), "缺 CSP");
    assert_eq!(
        h.get("x-content-type-options").unwrap(),
        "nosniff",
        "缺 nosniff，浏览器可能把资源当脚本执行"
    );
    assert_eq!(h.get("x-frame-options").unwrap(), "DENY");
    assert_eq!(h.get("referrer-policy").unwrap(), "no-referrer");
}

#[tokio::test]
async fn 关闭后端口立即释放() {
    let (base, _token, console) = spawn_server().await;
    let port: u16 = base.rsplit(':').next().unwrap().parse().unwrap();

    // 先确认在跑
    assert!(client().get(&base).send().await.is_ok());

    let (shutdown, _) = console.take_shutdown().await;
    shutdown.unwrap().send(true).unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // 同一端口应能重新 bind——否则说明监听没释放，
    // 用户改端口再开会莫名失败
    let rebind = tokio::net::TcpListener::bind(("127.0.0.1", port)).await;
    assert!(rebind.is_ok(), "关闭后端口未释放");
}

#[tokio::test]
async fn 超大登录请求体被拒绝() {
    let (base, _token, _c) = spawn_server().await;

    // 远超 1 KB 上限：不该被完整读进内存
    let huge = "x".repeat(64 * 1024);
    let res = client()
        .post(format!("{base}/api/login"))
        .json(&serde_json::json!({ "token": huge }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 401, "超限请求应被拒绝而不是照单全收");
}
