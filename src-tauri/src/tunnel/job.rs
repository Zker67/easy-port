//! Windows Job Object：应用被**强制结束**时连带干掉所有 cloudflared 子进程。
//!
//! 为什么需要它（不变量 5 的最后一道兜底）：
//!
//! 正常退出走 `RunEvent::Exit` → `shutdown_all()`，那条路径是好的。
//! 但强杀（任务管理器结束任务、`taskkill /F`、进程崩溃）不会给进程
//! 任何执行代码的机会——`RunEvent::Exit` 不触发、`Drop` 不运行，
//! `kill_on_drop(true)` 也随之失效（它依赖 Rust 跑析构函数）。
//!
//! 而 Windows 默认**不**把子进程绑定到父进程生命周期，于是 cloudflared
//! 变成孤儿继续在公网上挂着隧道——用户反馈的正是这个现象。
//!
//! Job Object 把这件事交给内核：所有子进程加入同一个作业对象，
//! 作业设了 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`，当持有它的最后一个句柄
//! 关闭时（进程无论怎么死，内核都会关掉它的句柄），系统自动终止作业内
//! 全部进程。这是唯一不依赖「被杀进程还能执行代码」的方案。
//!
//! 非 Windows 平台此模块为空实现：unix 下另有 `kill_pid` 与退出钩子，
//! 且 orphan 行为不同，不在本次问题范围内。

#[cfg(windows)]
mod imp {
    use std::sync::OnceLock;

    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, SetInformationJobObject,
        JobObjectExtendedLimitInformation, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
    };

    /// 句柄不是 Send/Sync，但它在进程内全局唯一且只读使用，包一层以便放进 OnceLock。
    struct JobHandle(HANDLE);
    // SAFETY: HANDLE 只是内核对象的索引值。这里只在 AssignProcessToJobObject
    // 里按值传给系统调用，不做任何解引用，跨线程共享是安全的。
    unsafe impl Send for JobHandle {}
    unsafe impl Sync for JobHandle {}

    static JOB: OnceLock<Option<JobHandle>> = OnceLock::new();

    /// 创建全局作业对象。进程生命周期内只建一次，失败返回 None（不阻断启动）。
    fn job() -> Option<HANDLE> {
        JOB.get_or_init(|| {
            // SAFETY: 标准 Win32 调用序列，参数均为合法值；
            // 失败时返回空句柄，下面显式检查。
            unsafe {
                let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if handle.is_null() {
                    eprintln!("[easy-port] 创建 Job Object 失败，强杀时可能残留子进程");
                    return None;
                }

                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags =
                    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

                let ok = SetInformationJobObject(
                    handle,
                    JobObjectExtendedLimitInformation,
                    &raw const info as *const _,
                    size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                );
                if ok == 0 {
                    eprintln!("[easy-port] 配置 Job Object 失败，强杀时可能残留子进程");
                    CloseHandle(handle);
                    return None;
                }

                Some(JobHandle(handle))
            }
        })
        .as_ref()
        .map(|h| h.0)
    }

    /// 把子进程加入作业对象。
    ///
    /// 失败只打日志不返回错误：隧道本身是好的，没必要因为兜底机制不可用
    /// 就让用户建不了映射——正常退出路径依然会清理。
    pub fn assign(pid: u32) {
        let Some(job) = job() else { return };

        // SAFETY: pid 来自刚 spawn 成功的子进程；拿到的句柄在函数结束前关闭。
        unsafe {
            let proc = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, pid);
            if proc.is_null() {
                eprintln!("[easy-port] 无法打开子进程 {pid}，跳过 Job 绑定");
                return;
            }
            if AssignProcessToJobObject(job, proc) == 0 {
                eprintln!("[easy-port] 子进程 {pid} 加入 Job 失败");
            }
            CloseHandle(proc);
        }
    }
}

#[cfg(not(windows))]
mod imp {
    /// 非 Windows 平台无需 Job Object，保持同名空实现让调用方无需 cfg。
    pub fn assign(_pid: u32) {}
}

pub use imp::assign;
