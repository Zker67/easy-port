//! 端到端验证访问计数：起一条真隧道，打几个请求，看计数对不对得上。
//!
//! 需要外网与本机 3080 上有服务；条件不满足时跳过而非失败。

use easy_port_lib::tunnel::{cloudflared, metrics};

#[tokio::test]
async fn 访问计数与实际请求数一致() {
    // 本机得先有个服务可映射
    if std::net::TcpStream::connect("127.0.0.1:3080").is_err() {
        eprintln!("跳过：本机 3080 无服务");
        return;
    }

    let spawned = match cloudflared::spawn(3080).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("跳过：建立隧道失败（可能无外网）：{e}");
            return;
        }
    };

    let port = spawned.metrics_port.expect("必须解析出 metrics 端口");
    let url = spawned.public_url.clone();
    let mut child = spawned.child;

    // 先读一次基线：隧道自身的健康检查等可能已产生请求
    let base = metrics::fetch(port).await.expect("应能抓到指标");

    // 新建的 Quick Tunnel 需要几秒才在边缘生效，立刻打会 502/530。
    // 先重试直到通，再开始计数——否则测的是「请求失败了几次」。
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap();
    let mut warm = false;
    for _ in 0..15 {
        match client.get(&url).send().await {
            Ok(r) if r.status().is_success() => {
                warm = true;
                break;
            }
            _ => tokio::time::sleep(std::time::Duration::from_secs(2)).await,
        }
    }
    if !warm {
        // 隧道没在边缘生效，属环境问题而非代码缺陷（网络受限、CF 边缘抖动）。
        // 跳过而不是判失败——否则这个测试会在网络不好时误报。
        eprintln!("跳过：隧道 30 秒内未在边缘生效，无法验证计数");
        let _ = child.kill().await;
        return;
    }

    // 重新取基线：上面的预热请求也会被计入
    let base = metrics::fetch(port).await.expect("应能抓到指标");

    let mut sent = 0;
    for _ in 0..3 {
        match client.get(&url).send().await {
            Ok(r) if r.status().is_success() => sent += 1,
            Ok(r) => eprintln!("请求返回 {}", r.status()),
            Err(e) => eprintln!("请求失败：{e}"),
        }
    }
    assert!(sent > 0, "预热已通过但正式请求全失败，说明隧道中途断了");
    // 指标聚合有延迟，等一下再读
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let after = metrics::fetch(port).await.expect("应能抓到指标");
    let _ = child.kill().await;

    assert!(
        after.total_requests >= base.total_requests + sent,
        "计数没跟上：基线 {} + 发出 {sent}，实际 {}",
        base.total_requests,
        after.total_requests
    );
    assert_eq!(after.connections, 1, "应有一条到边缘的连接");
}
