//! 读取 cloudflared 的本地指标端口，得到「这条映射被访问了多少次」。
//!
//! cloudflared 用 `--metrics` 暴露一个 Prometheus 格式的端点。我们传
//! `127.0.0.1:0` 让系统分配端口——它默认只在 20241~20245 里挑，最多撑 5 条隧道，
//! 而本应用的映射条数没有上限。实际端口由 cloudflared 打印在 stderr 上，
//! 与公网链接同一路输出——注意**端口行在链接行之后**（实测链接第 5 行、
//! 端口第 20 行），所以 spawn 里不能取到链接就停止读取。
//!
//! **只取计数，不取延迟**：`cloudflared_proxy_connect_latency` 统计的是
//! TCP/WebSocket 代理连接，普通 HTTP 请求不计入，实测恒为 0。
//! 唯一有值的延迟是 cloudflared 注册到边缘的耗时，只在建立时记一次，
//! 对「这条映射用得多不多」没有意义，因此不采集，免得在界面上显示一个假的 0ms。

use std::time::Duration;

use regex::Regex;
use serde::{Deserialize, Serialize};

/// 单条隧道的访问统计。
///
/// 这些值是 **cloudflared 进程内累计**，隧道一重连就归零。
/// 因此语义是「本次连接以来」，不是「这个端口历史总访问量」。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TunnelMetrics {
    /// 总请求数。
    pub total_requests: u64,
    /// 出错的请求数。
    pub request_errors: u64,
    /// 当前正在处理的并发请求数。
    pub concurrent_requests: u64,
    /// 到 Cloudflare 边缘的连接数，0 表示这条隧道此刻其实是断的。
    pub connections: u64,
}

/// 从 cloudflared 的 stderr 里解析出 metrics 端口。
///
/// 目标行形如：`INF Starting metrics server on 127.0.0.1:55216/metrics`
pub fn parse_metrics_port(line: &str) -> Option<u16> {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"metrics server on 127\.0\.0\.1:(\d{1,5})")
            .expect("内置正则必定可编译")
    });
    re.captures(line)?.get(1)?.as_str().parse().ok()
}

/// 从 Prometheus 文本里取一个无标签指标的值。
///
/// 格式是 `名字 值`，一行一个。只认行首完整匹配的名字，
/// 否则 `cloudflared_tunnel_total_requests` 会被
/// `cloudflared_tunnel_total_requests_foo` 这类前缀相同的行干扰。
fn scrape(text: &str, name: &str) -> u64 {
    text.lines()
        .filter_map(|line| {
            let rest = line.strip_prefix(name)?;
            // 名字后必须紧跟空白，排除前缀相同的其他指标
            if !rest.starts_with(' ') {
                return None;
            }
            rest.trim().parse::<f64>().ok()
        })
        .next()
        // 指标是浮点文本（可能是 "3" 也可能是 "3.0"），截断取整
        .map(|v| v as u64)
        .unwrap_or(0)
}

/// 抓一次指标。失败返回 None——指标是锦上添花，拿不到不该影响隧道本身。
pub async fn fetch(port: u16) -> Option<TunnelMetrics> {
    let client = reqwest::Client::builder()
        // 本机回环，正常是毫秒级；给足 2 秒避免偶发抖动误判为失败
        .timeout(Duration::from_secs(2))
        .build()
        .ok()?;

    let text = client
        .get(format!("http://127.0.0.1:{port}/metrics"))
        .send()
        .await
        .ok()?
        .text()
        .await
        .ok()?;

    Some(TunnelMetrics {
        total_requests: scrape(&text, "cloudflared_tunnel_total_requests"),
        request_errors: scrape(&text, "cloudflared_tunnel_request_errors"),
        concurrent_requests: scrape(
            &text,
            "cloudflared_tunnel_concurrent_requests_per_tunnel",
        ),
        connections: scrape(&text, "cloudflared_tunnel_ha_connections"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 能从日志行解析出_metrics_端口() {
        let line = "2026-09-11T06:46:26Z INF Starting metrics server on 127.0.0.1:55216/metrics";
        assert_eq!(parse_metrics_port(line), Some(55216));
    }

    #[test]
    fn 无关日志行不会误判为端口() {
        // 这一行也含 "metrics" 字样，但不是启动行，不能匹配
        let settings = "INF Settings: map[metrics:127.0.0.1:0 url:http://localhost:3080]";
        assert_eq!(parse_metrics_port(settings), None);
        assert_eq!(parse_metrics_port("INF Registered tunnel connection"), None);
    }

    #[test]
    fn 按整行名字取值不被同前缀指标干扰() {
        // 真实端点里确实同时存在这两个名字，前缀完全相同
        let text = "\
cloudflared_tunnel_total_requests 3
cloudflared_tunnel_request_errors 0
";
        assert_eq!(scrape(text, "cloudflared_tunnel_total_requests"), 3);
        assert_eq!(scrape(text, "cloudflared_tunnel_request_errors"), 0);
    }

    #[test]
    fn 带标签的指标不参与无标签取值() {
        // response_by_code 带 {status_code="200"}，不该被当成 total_requests
        let text = "\
cloudflared_tunnel_response_by_code{status_code=\"200\"} 7
cloudflared_tunnel_total_requests 3
";
        assert_eq!(scrape(text, "cloudflared_tunnel_total_requests"), 3);
    }

    #[test]
    fn 缺失的指标按零处理() {
        // 隧道刚起来时某些指标还没出现，不能因此报错
        assert_eq!(scrape("", "cloudflared_tunnel_total_requests"), 0);
    }

    #[test]
    fn 浮点写法能取整() {
        // Prometheus 的 gauge 可能写成 3.0
        assert_eq!(scrape("cloudflared_tunnel_ha_connections 1.0", "cloudflared_tunnel_ha_connections"), 1);
    }
}
