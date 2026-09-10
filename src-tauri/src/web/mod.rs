//! Web 远程控制台：内嵌 HTTP 服务 + 特别端口。
//!
//! 与普通映射的本质区别：普通映射暴露的是**用户自己的服务**，
//! 这里暴露的是 **Easy Port 自己的控制面**——拿到 URL 与 token 的人
//! 能在本机开关任意端口的公网映射。安全设计见 `auth` 模块与
//! plans/2026-09-10-web-console/02-security.md。

pub mod assets;
pub mod auth;
pub mod commands;
pub mod console;
pub mod server;

use tokio::process::Child;

use console::{SharedConsole, WebStatus};

/// 关闭控制台：停服 + 杀隧道 + 作废会话。
///
/// 三条关闭路径共用（用户手动、应用退出、隧道崩溃）。
/// 应用退出时**必须**调用：控制台的隧道不在 registry 里，
/// `shutdown_all` 收不到它（不变量 5）。
pub async fn shutdown_console(console: &SharedConsole) {
    let (shutdown, pid) = console.take_shutdown().await;
    if let Some(tx) = shutdown {
        // 忽略发送失败：接收端已退出说明服务本来就没在跑
        let _ = tx.send(true);
    }
    if let Some(pid) = pid {
        crate::tunnel::registry::kill_pid(pid).await;
    }
    console.auth.lock().await.revoke_all();
}

/// 监视控制台隧道的子进程。
///
/// 隧道死了但 HTTP 服务还活着，会留下一个「本机监听着、外部访问不到」的
/// 半死状态——用户看到「运行中」却打不开。因此这里在隧道退出时
/// **主动把服务一并停掉**，状态转失败。
pub fn watch_tunnel(console: SharedConsole, mut child: Child) {
    tokio::spawn(async move {
        let _ = child.wait().await;

        // 已经不在运行态说明是用户主动关的，stop 路径已处理好一切
        if !console.is_running().await {
            return;
        }

        let (shutdown, _) = console.take_shutdown().await;
        if let Some(tx) = shutdown {
            let _ = tx.send(true);
        }
        console.auth.lock().await.revoke_all();
        console
            .set_status(WebStatus::Failed(
                "隧道进程意外退出，控制台已停止。重新开启即可".into(),
            ))
            .await;
    });
}
