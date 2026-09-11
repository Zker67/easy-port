//! Job Object 的验证辅助程序，供 `tests/job_object.rs` 调用。
//!
//! 单独做成 example 而不是写在测试里，是因为要验的是「进程被强杀时」的行为——
//! 测试进程没法杀自己再继续断言。这里由测试从外部 taskkill /F 掉本进程，
//! 再去检查子进程是否被系统连带终止。
//!
//! 用法：`job_orphan_probe`，向 stdout 打印子进程 pid 后挂起等待被杀。

fn main() {
    // 起一个长睡的子进程代替 cloudflared：验的是 Job 绑定本身，与被绑的是谁无关
    let child = std::process::Command::new("cmd")
        .args(["/C", "ping -n 120 127.0.0.1 > nul"])
        .spawn()
        .expect("无法启动子进程");

    let pid = child.id();
    easy_port_lib::tunnel::job::assign(pid);

    // 告诉测试子进程的 pid，然后挂起等着被强杀
    println!("{pid}");
    use std::io::Write;
    std::io::stdout().flush().unwrap();

    std::thread::sleep(std::time::Duration::from_secs(120));
}
