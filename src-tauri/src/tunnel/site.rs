//! 探测被映射的本机服务的站点信息（网页标题与 favicon）。
//!
//! 只访问 `127.0.0.1:<port>`，不碰公网链接——公网请求会绕一圈 Cloudflare，
//! 既慢又会在边缘留下访问记录。
//!
//! 拿不到就返回 None：映射的目标未必是网页（可能是 API、WebSocket、静态文件服务），
//! 探测失败是常态，不是错误，不应打扰用户。

use std::sync::OnceLock;
use std::time::Duration;

use regex::Regex;

use super::provider::SiteInfo;

/// 单次探测的超时。本机请求应当很快，给足 2 秒已很宽松。
const PROBE_TIMEOUT: Duration = Duration::from_secs(2);

/// 只读取响应前若干字节：`<title>` 必在 `<head>` 里，
/// 没必要为一个标题下载整个页面（可能是几 MB 的 SPA）。
const MAX_HTML_BYTES: usize = 64 * 1024;

/// favicon 体积上限，超过就不内联（避免把大图塞进 IPC 与内存）。
const MAX_ICON_BYTES: usize = 128 * 1024;

fn title_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // (?is): 忽略大小写 + 让 . 匹配换行，标题常被格式化到多行
        Regex::new(r"(?is)<title[^>]*>(.*?)</title>").expect("内置正则必定可编译")
    })
}

/// 从 HTML 里提取 `<link rel="...icon..." href="...">` 的 href。
fn icon_href_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?is)<link[^>]+rel\s*=\s*["'][^"']*icon[^"']*["'][^>]*>"#)
            .expect("内置正则必定可编译")
    })
}

fn href_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?is)href\s*=\s*["']([^"']+)["']"#).expect("内置正则必定可编译")
    })
}

/// 极简 HTML 实体反转义，只处理标题里常见的几个。
fn unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

/// 探测指定本机端口上的站点信息。
///
/// 任何失败都返回 None——目标未必是网页，探测不到属正常情况。
pub async fn probe(port: u16) -> Option<SiteInfo> {
    let client = reqwest::Client::builder()
        .timeout(PROBE_TIMEOUT)
        // 本机自签证书很常见，探测标题不值得为此失败
        .danger_accept_invalid_certs(true)
        .build()
        .ok()?;

    let base = format!("http://127.0.0.1:{port}");
    let resp = client.get(&base).send().await.ok()?;

    // 非 HTML 就没有标题可言（API 服务等），直接放弃。
    let is_html = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.contains("html"));
    if !is_html {
        return None;
    }

    let body = resp.text().await.ok()?;
    let head = &body[..body.len().min(MAX_HTML_BYTES)];

    let title = title_regex()
        .captures(head)
        .and_then(|c| c.get(1))
        .map(|m| unescape(m.as_str()).trim().to_string())
        .filter(|s| !s.is_empty());

    let icon = fetch_icon(&client, &base, head).await;

    if title.is_none() && icon.is_none() {
        return None;
    }
    Some(SiteInfo { title, icon })
}

/// 取 favicon 并编码为 data URI。先试 HTML 里声明的，再退回 /favicon.ico。
async fn fetch_icon(client: &reqwest::Client, base: &str, head: &str) -> Option<String> {
    let declared = icon_href_regex()
        .find(head)
        .and_then(|m| href_regex().captures(m.as_str()))
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string());

    let mut candidates = Vec::new();
    if let Some(href) = declared {
        // data: 开头的直接就是图标本身，无需再请求
        if href.starts_with("data:") {
            return Some(href);
        }
        candidates.push(resolve_url(base, &href));
    }
    candidates.push(format!("{base}/favicon.ico"));

    for url in candidates {
        let Ok(resp) = client.get(&url).send().await else {
            continue;
        };
        if !resp.status().is_success() {
            continue;
        }
        let mime = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("image/x-icon")
            .split(';')
            .next()
            .unwrap_or("image/x-icon")
            .to_string();

        let Ok(bytes) = resp.bytes().await else {
            continue;
        };
        if bytes.is_empty() || bytes.len() > MAX_ICON_BYTES {
            continue;
        }

        use base64::Engine as _;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        return Some(format!("data:{mime};base64,{b64}"));
    }
    None
}

/// 把 href 解析为绝对 URL（只处理本机场景需要的三种形式）。
fn resolve_url(base: &str, href: &str) -> String {
    if href.starts_with("http://") || href.starts_with("https://") {
        href.to_string()
    } else if let Some(rest) = href.strip_prefix('/') {
        format!("{base}/{rest}")
    } else {
        format!("{base}/{href}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 能提取标题并反转义() {
        let html = r#"<html><head><title>My &amp; App</title></head>"#;
        let t = title_regex()
            .captures(html)
            .and_then(|c| c.get(1))
            .map(|m| unescape(m.as_str()).trim().to_string());
        assert_eq!(t.as_deref(), Some("My & App"));
    }

    #[test]
    fn 标题跨行也能提取() {
        let html = "<head><title>\n  Dev Server\n</title></head>";
        let t = title_regex()
            .captures(html)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().trim().to_string());
        assert_eq!(t.as_deref(), Some("Dev Server"));
    }

    #[test]
    fn 能提取声明的图标地址() {
        let html = r#"<link rel="shortcut icon" href="/static/fav.png">"#;
        let href = icon_href_regex()
            .find(html)
            .and_then(|m| href_regex().captures(m.as_str()))
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string());
        assert_eq!(href.as_deref(), Some("/static/fav.png"));
    }

    #[test]
    fn 相对地址解析为绝对地址() {
        let base = "http://127.0.0.1:3000";
        assert_eq!(resolve_url(base, "/a.png"), "http://127.0.0.1:3000/a.png");
        assert_eq!(resolve_url(base, "a.png"), "http://127.0.0.1:3000/a.png");
        assert_eq!(
            resolve_url(base, "http://x/a.png"),
            "http://x/a.png",
            "绝对地址应原样保留"
        );
    }
}
