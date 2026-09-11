//! 验证「应用被强制结束时，子进程被连带终止」（不变量 5 的兜底）。
//!
//! 这条路径**无法用普通单元测试覆盖**：要验的正是「进程没机会执行任何代码」
//! 的情形，测试进程不可能杀掉自己再继续断言。因此借一个 example 辅助程序：
//! 它绑好子进程后挂起，由本测试从外部 `taskkill /F` 掉它，再检查子进程状态。
//!
//! 用户实测反馈过「强制退出软件之后映射还在」——正常退出走 RunEvent::Exit
//! 是好的，强杀则完全绕过它。这个测试就是那次回归的守门人。

#![cfg(windows)]

use std::process::{Command, Stdio};
use std::time::Duration;

/// 进程当前是否存在。用 tasklist 过滤指定 pid，输出里出现该 pid 即存活。
fn alive(pid: u32) -> bool {
    let out = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH"])
        .output()
        .expect("tasklist 不可用");
    let text = String::from_utf8_lossy(&out.stdout);
    text.contains(&pid.to_string())
}

fn kill(pid: u32) {
    let _ = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[test]
fn 父进程被强杀后子进程不会变成孤儿() {
    // example 由 cargo 在 test 之前构建，路径与测试二进制同级的 examples/ 下
    let exe = std::env::current_exe().expect("取不到测试二进制路径");
    let probe = exe
        .parent()
        .and_then(|p| p.parent()) // deps/ → debug|release/
        .map(|p| p.join("examples").join("job_orphan_probe.exe"))
        .expect("拼不出 example 路径");

    if !probe.is_file() {
        // 没构建 example 时跳过而非失败：`cargo test --test job_object` 单独跑
        // 不会自动带上 examples，提示用户怎么跑完整的
        eprintln!(
            "跳过：未找到 {}，请用 `cargo test --examples --test job_object` 或先 `cargo build --examples`",
            probe.display()
        );
        return;
    }

    let mut parent = Command::new(&probe)
        .stdout(Stdio::piped())
        .spawn()
        .expect("无法启动 probe");

    // 读出子进程 pid（probe 打印后才挂起，读到即说明绑定已完成）
    let child_pid: u32 = {
        use std::io::{BufRead, BufReader};
        let stdout = parent.stdout.take().expect("probe 无 stdout");
        let mut line = String::new();
        BufReader::new(stdout)
            .read_line(&mut line)
            .expect("读不到 probe 输出");
        line.trim().parse().expect("probe 输出的不是 pid")
    };

    assert!(alive(child_pid), "前提不成立：子进程应当已经起来了");

    // 强杀父进程：不给它任何执行清理代码的机会，
    // 这正是 RunEvent::Exit 与 Drop 都指望不上的那种死法
    let parent_pid = parent.id();
    kill(parent_pid);

    // 内核终止作业内进程需要一点时间，轮询而不是睡死
    let mut still_alive = true;
    for _ in 0..40 {
        std::thread::sleep(Duration::from_millis(100));
        if !alive(child_pid) {
            still_alive = false;
            break;
        }
    }

    if still_alive {
        // 别把孤儿留给后续测试和用户的机器
        kill(child_pid);
        panic!(
            "子进程 {child_pid} 在父进程被强杀后仍存活——Job Object 没生效，\
             这会让用户强制退出应用后隧道继续挂在公网上"
        );
    }
}
