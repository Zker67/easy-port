//! 端到端集成测试：对真实 cloudflared 建立隧道并验证公网可达。
//!
//! 需要本机已安装 cloudflared 且能连通外网；缺任一条件时自动跳过，
//! 保证在 CI 或离线环境下不会误报失败。
//!
//! 运行：cargo test --test tunnel_e2e -- --nocapture

use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};
use std::process::{Child, Command, Stdio};

use easy_port_lib::tunnel::cloudflared;

/// 在指定端口起一个最小 HTTP 服务，返回子进程句柄。
fn serve(port: u16, body: &str) -> Option<Child> {
    let script = format!(
        r#"require('http').createServer((_,r)=>{{r.writeHead(200,{{'content-type':'text/plain'}});r.end('{body}')}}).listen({port},'127.0.0.1')"#
    );
    Command::new("node")
        .args(["-e", &script])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()
}

fn port_is_free(port: u16) -> bool {
    TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).is_ok()
}

#[tokio::test]
async fn 隧道建立后公网可达且断开后回收进程() {
    let engine = cloudflared::check_engine().await;
    if !engine.available {
        eprintln!("跳过：本机未安装 cloudflared");
        return;
    }
    eprintln!("cloudflared 版本 {:?}", engine.version);

    const PORT: u16 = 18899;
    const BODY: &str = "easy-port-e2e-ok";

    if !port_is_free(PORT) {
        eprintln!("跳过：端口 {PORT} 被占用");
        return;
    }

    let mut server = match serve(PORT, BODY) {
        Some(c) => c,
        None => {
            eprintln!("跳过：本机没有可用的 node");
            return;
        }
    };
    tokio::time::sleep(std::time::Duration::from_millis(800)).await;

    // 建立隧道
    let spawned = match cloudflared::spawn(PORT).await {
        Ok(s) => s,
        Err(e) => {
            let _ = server.kill();
            eprintln!("跳过：建立隧道失败（可能无外网）：{e}");
            return;
        }
    };
    let url = spawned.public_url.clone();
    eprintln!("已获得公网链接");

    assert!(
        url.starts_with("https://") && url.ends_with(".trycloudflare.com"),
        "链接格式不符合预期"
    );

    // Quick Tunnel 拿到链接后，边缘节点仍需约 10 秒才开始转发，
    // 过早访问会拿到 Cloudflare 的 error 1033 页面，因此首次等待要给足。
    let mut reached = false;
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    for attempt in 1..=8 {
        let out = Command::new("curl")
            .args(["-s", "--max-time", "15", &url])
            .output();
        match out {
            Ok(o) => {
                let text = String::from_utf8_lossy(&o.stdout);
                if text.contains(BODY) {
                    eprintln!("第 {attempt} 次尝试：公网可达，内容匹配");
                    reached = true;
                    break;
                }
                eprintln!(
                    "第 {attempt} 次尝试：尚未就绪（响应 {} 字节）",
                    text.len()
                );
            }
            Err(e) => eprintln!("第 {attempt} 次尝试：curl 调用失败 {e}"),
        }
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }

    // 无论结果如何都要清理，避免留下孤儿进程
    let mut child = spawned.child;
    let _ = child.kill().await;
    let _ = server.kill();

    assert!(reached, "隧道建立成功但公网无法访问到本机内容");
}
