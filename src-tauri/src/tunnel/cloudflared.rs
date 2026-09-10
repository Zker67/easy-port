//! cloudflared Quick Tunnel 的具体实现。
//!
//! 关键事实（踩坑点，勿改）：
//! 1. cloudflared 把公网链接打印在 **stderr**，不是 stdout。
//! 2. Quick Tunnel 无需 Cloudflare 账号或 token。
//! 3. Windows 下必须设置 CREATE_NO_WINDOW，否则每条隧道会弹一个黑色控制台窗口。
//!
//! 可执行文件来源（M6 起）：优先用随包分发的 sidecar，缺失时回退到 PATH。
//! 回退不可省：开发期（`tauri dev`）与用户自行安装的场景都依赖它。

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::OnceLock;

use regex::Regex;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};

use super::provider::{EngineStatus, TunnelError};

/// 等待公网链接的最长时间（秒）。cloudflared 正常在 2~5 秒内给出链接。
///
/// 前端等待提示与超时错误文案都引用这个值，改动它会同步反映到界面上。
pub const URL_TIMEOUT_SECS: u64 = 30;

const URL_TIMEOUT: Duration = Duration::from_secs(URL_TIMEOUT_SECS);

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

/// 编译期嵌入的 cloudflared 二进制。
///
/// 由 `build.rs` 保证 `src-tauri/binaries/cloudflared-<triple><ext>` 存在；
/// 缺失时构建直接失败，不会产出一个「看似正常但没有引擎」的包。
///
/// 代价：主程序体积增加约 53 MB。这是用户明确要求的「一体化」形态。
#[cfg(feature = "embed-cloudflared")]
const EMBEDDED: &[u8] = include_bytes!(env!("EASY_PORT_CLOUDFLARED"));

/// 实际使用的 cloudflared 路径，由 `lib.rs` 在 setup 阶段解析后写入。
///
/// 用 OnceLock 而非每次解析：路径在进程生命周期内不变，
/// 且 tunnel 模块不应依赖 tauri 的 AppHandle。
static SIDECAR: OnceLock<Option<PathBuf>> = OnceLock::new();

/// 登记 cloudflared 路径。仅在应用启动时调用一次；文件不存在时传 None。
pub fn set_sidecar(path: Option<PathBuf>) {
    let _ = SIDECAR.set(path.filter(|p| p.is_file()));
}

/// 把内嵌的 cloudflared 释放到 app data 目录，返回其路径。
///
/// Windows 无法直接从内存执行，必须落盘一次。已存在且大小一致时跳过写入，
/// 避免每次启动都写 53 MB。
#[cfg(feature = "embed-cloudflared")]
pub fn extract_embedded(dir: &std::path::Path) -> std::io::Result<PathBuf> {
    let name = if cfg!(windows) {
        "cloudflared.exe"
    } else {
        "cloudflared"
    };
    let target = dir.join(name);

    // 大小一致即认为是同一份，跳过重写（也避免覆盖正在运行的进程映像）。
    if let Ok(meta) = std::fs::metadata(&target) {
        if meta.len() == EMBEDDED.len() as u64 {
            return Ok(target);
        }
    }

    std::fs::create_dir_all(dir)?;
    // 先写临时文件再 rename：中途失败不会留下半个二进制。
    let tmp = target.with_extension("tmp");
    std::fs::write(&tmp, EMBEDDED)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))?;
    }

    // Windows 下目标若被占用会失败，此时沿用已有文件。
    if std::fs::rename(&tmp, &target).is_err() {
        let _ = std::fs::remove_file(&tmp);
        if !target.is_file() {
            return Err(std::io::Error::other("无法释放内置 cloudflared"));
        }
    }
    Ok(target)
}

/// 解析实际要执行的 cloudflared：优先 sidecar，回退 PATH 上的同名命令。
fn program() -> String {
    match SIDECAR.get().and_then(|o| o.as_ref()) {
        Some(path) => path.to_string_lossy().into_owned(),
        None => "cloudflared".into(),
    }
}

/// 当前是否在用随包分发的二进制。
pub fn is_bundled() -> bool {
    SIDECAR.get().and_then(|o| o.as_ref()).is_some()
}

fn base_command(program: &str) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// 定位 cloudflared 可执行文件，供设置页展示来源。
///
/// 用了 sidecar 就直接返回其路径；否则用系统自带的 where / which 查 PATH，
/// 避免自己解析 PATH 与扩展名规则。
async fn locate_executable() -> Option<String> {
    if let Some(path) = SIDECAR.get().and_then(|o| o.as_ref()) {
        return Some(path.to_string_lossy().into_owned());
    }
    #[cfg(windows)]
    let (program, arg) = ("where", "cloudflared");
    #[cfg(not(windows))]
    let (program, arg) = ("which", "cloudflared");

    let out = base_command(program)
        .arg(arg)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
        .ok()?;

    if !out.status.success() {
        return None;
    }
    // where 可能返回多行，取第一条命中。
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// 检查本机是否有可用的 cloudflared。
pub async fn check_engine() -> EngineStatus {
    let output = base_command(&program())
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
                path: locate_executable().await,
                bundled: is_bundled(),
            }
        }
        _ => EngineStatus {
            available: false,
            version: None,
            engine: "cloudflared".into(),
            path: None,
            bundled: is_bundled(),
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
    let mut child = base_command(&program())
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
            Err(TunnelError::UrlTimeout(URL_TIMEOUT_SECS))
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

    #[test]
    fn 未登记_sidecar_时回退到_PATH() {
        // SIDECAR 是进程级 OnceLock，测试里不能真正 set；
        // 这里验证的是「未 set 时」的默认行为——即开发期与用户自装场景。
        if SIDECAR.get().is_none() {
            assert_eq!(program(), "cloudflared", "未登记时必须回退到 PATH 上的命令");
            assert!(!is_bundled());
        }
    }

    #[test]
    fn 不存在的_sidecar_路径不会被采纳() {
        // set_sidecar 用 is_file() 过滤，传一个不存在的路径应等价于 None。
        let bogus = PathBuf::from("Z:/definitely/not/here/cloudflared.exe");
        assert!(!bogus.is_file(), "测试前提：该路径不应存在");
        // 直接验证过滤条件本身，避免污染进程级 OnceLock。
        assert!(Some(bogus).filter(|p| p.is_file()).is_none());
    }

    #[test]
    fn 超时文案中的秒数与实际超时一致() {
        // 防止「改了常量忘了改文案」：文案里的秒数必须来自 URL_TIMEOUT_SECS。
        let msg = TunnelError::UrlTimeout(URL_TIMEOUT_SECS).to_string();
        assert!(
            msg.contains(&URL_TIMEOUT_SECS.to_string()),
            "超时提示应包含实际秒数，实际为：{msg}"
        );
        assert_eq!(URL_TIMEOUT.as_secs(), URL_TIMEOUT_SECS);
    }
}
