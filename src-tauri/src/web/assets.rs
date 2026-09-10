//! Web 端静态资源。
//!
//! 资源用 `include_dir!` 编进二进制，按固定映射表提供，
//! **不做任何文件系统路径拼接**——从根上杜绝路径穿越（`../../etc/passwd` 之类）。
//! 这不是「小心处理用户输入」的问题，而是让这类攻击在结构上不可能。

use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use include_dir::{include_dir, Dir};

/// Web 端构建产物。由 `npm run build:web` 生成到 `dist-web/`。
static WEB_DIST: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../dist-web");

/// 提供静态资源；未命中的路径一律回退到 index.html。
///
/// 回退而非 404 是因为 Web 端是单页应用，刷新任意路径都该拿到入口页。
pub async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    match WEB_DIST.get_file(path) {
        Some(file) => file_response(path, file.contents()),
        // 单页应用回退
        None => match WEB_DIST.get_file("index.html") {
            Some(index) => file_response("index.html", index.contents()),
            None => (StatusCode::NOT_FOUND, "Web 控制台资源缺失").into_response(),
        },
    }
}

fn file_response(path: &str, body: &'static [u8]) -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, content_type(path))],
        body,
    )
        .into_response()
}

/// 按扩展名给出 MIME。
///
/// 用白名单而不是猜测：配合 `X-Content-Type-Options: nosniff`，
/// 避免浏览器把某个资源当成脚本执行。
fn content_type(path: &str) -> &'static str {
    match path.rsplit_once('.').map(|(_, ext)| ext) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 按扩展名给出正确的_mime() {
        assert_eq!(content_type("index.html"), "text/html; charset=utf-8");
        assert_eq!(content_type("assets/app.js"), "text/javascript; charset=utf-8");
        assert_eq!(content_type("assets/app.css"), "text/css; charset=utf-8");
        // 未知扩展名不猜测，交给 nosniff 兜底
        assert_eq!(content_type("weird.xyz"), "application/octet-stream");
        assert_eq!(content_type("noext"), "application/octet-stream");
    }

    #[test]
    fn 路径穿越取不到任何东西() {
        // include_dir 的 get_file 是按打包时的相对路径查表，
        // 不经过文件系统，因此这类路径根本不可能命中
        assert!(WEB_DIST.get_file("../../../etc/passwd").is_none());
        assert!(WEB_DIST.get_file("../Cargo.toml").is_none());
        assert!(WEB_DIST.get_file("/etc/passwd").is_none());
    }

    #[test]
    fn 打包进来的资源里有入口页() {
        assert!(
            WEB_DIST.get_file("index.html").is_some(),
            "dist-web 未构建或缺少 index.html：先跑 npm run build:web"
        );
    }
}
