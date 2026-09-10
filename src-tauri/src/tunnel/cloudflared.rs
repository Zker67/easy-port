//! cloudflared Quick Tunnel 的具体实现。
//!
//! 关键事实（踩坑点，勿改）：
//! 1. cloudflared 把公网链接打印在 **stderr**，不是 stdout。
//! 2. Quick Tunnel 无需 Cloudflare 账号或 token。
//! 3. Windows 下必须设置 CREATE_NO_WINDOW，否则每条隧道会弹一个黑色控制台窗口。

use std::process::Stdio;
use std::sync::OnceLock;

use regex::Regex;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};

use super::provider::{EngineStatus, TunnelError};

/// 等待公网链接的最长时间。cloudflared 正常在 2~5 秒内给出链接。
const URL_TIMEOUT: Duration = Duration::from_secs(30);

/// Windows: 不为子进程创建控制台窗口。
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// 匹配 Quick Tunnel 分配的链接，形如 https://xxx-yyy-zzz.trycloudflare.com
fn url_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"https://[a-z0-9-]+\.trycloudflare\.com")
            .expect("内置正则必定可编译")
    })
}

fn base_command(program: &str) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// 检查本机是否有可用的 cloudflared。
pub async fn check_engine() -> EngineStatus {
    let output = base_command("cloudflared")
        .arg("--version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await;

    match output {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout);
            EngineStatus {
                available: true,
                version: text.split_whitespace().nth(2).map(str::to_string),
                engine: "cloudflared".into(),
            }
        }
        _ => EngineStatus {
            available: false,
            version: None,
            engine: "cloudflared".into(),
        },
    }
}

/// 已拉起的 cloudflared 进程及其公网链接。
pub struct SpawnedTunnel {
    pub child: Child,
    pub public_url: String,
}

/// 为指定本机端口拉起一条 Quick Tunnel，等待并返回公网链接。
///
/// 失败时会确保子进程被清理，不留孤儿进程（不变量 5）。
pub async fn spawn(port: u16) -> Result<SpawnedTunnel, TunnelError> {
    let mut child = base_command("cloudflared")
        .args([
            "tunnel",
            "--url",
            &format!("http://localhost:{port}"),
            // 关掉自动更新，避免运行期间进程被替换
            "--no-autoupdate",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                TunnelError::EngineMissing
            } else {
                TunnelError::SpawnFailed(e.to_string())
            }
        })?;

    // 链接在 stderr；同时读 stdout 防止管道写满导致子进程阻塞。
    let stderr = child.stderr.take().ok_or_else(|| {
        TunnelError::SpawnFailed("无法捕获 cloudflared 输出".into())
    })?;
    let stdout = child.stdout.take();

    let (tx, mut rx) = mpsc::channel::<String>(1);

    tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if let Some(m) = url_regex().find(&line) {
                let _ = tx.send(m.as_str().to_string()).await;
                break;
            }
        }
    });

    if let Some(out) = stdout {
        tokio::spawn(async move {
            let mut lines = BufReader::new(out).lines();
            while let Ok(Some(_)) = lines.next_line().await {}
        });
    }

    match timeout(URL_TIMEOUT, rx.recv()).await {
        Ok(Some(public_url)) => Ok(SpawnedTunnel { child, public_url }),
        // 通道关闭或超时：两种情况都必须回收子进程
        Ok(None) => {
            let _ = child.kill().await;
            Err(TunnelError::SpawnFailed(
                "cloudflared 未输出公网链接即退出".into(),
            ))
        }
        Err(_) => {
            let _ = child.kill().await;
            Err(TunnelError::UrlTimeout)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 能从日志行中提取链接() {
        let line = "2026-09-10T04:00:00Z INF |  https://fuzzy-panda-quick-test.trycloudflare.com  |";
        let found = url_regex().find(line).map(|m| m.as_str());
        assert_eq!(
            found,
            Some("https://fuzzy-panda-quick-test.trycloudflare.com")
        );
    }

    #[test]
    fn 忽略无关链接() {
        let line = "INF See https://developers.cloudflare.com/argo-tunnel for docs";
        assert!(url_regex().find(line).is_none());
    }
}
