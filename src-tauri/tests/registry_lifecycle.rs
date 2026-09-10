//! 集成测试：registry 的进程生命周期与持久化（M4）。
//!
//! 这里用真实子进程（一个长睡的 node / sleep 进程）代替 cloudflared，
//! 只验证 registry 自身的行为：崩溃感知、主动停止不误报、落盘往返。
//! 与 `tunnel_e2e` 分工不同，本文件不需要外网。
//!
//! 运行：cargo test --test registry_lifecycle -- --nocapture

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use easy_port_lib::store::StateStore;
use easy_port_lib::tunnel::provider::{Tunnel, TunnelStatus};
use easy_port_lib::tunnel::registry::TunnelRegistry;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "easy-port-it-{}-{}-{tag}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 拉起一个会存活一段时间的子进程，用于代替 cloudflared。
fn spawn_dummy() -> Option<tokio::process::Child> {
    let mut cmd = tokio::process::Command::new("node");
    cmd.args(["-e", "setTimeout(()=>{}, 600000)"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);
    cmd.spawn().ok()
}

fn tunnel(port: u16) -> Tunnel {
    Tunnel {
        id: uuid::Uuid::new_v4().to_string(),
        port,
        label: Some("it".into()),
        public_url: Some("https://placeholder.trycloudflare.com".into()),
        status: TunnelStatus::Running,
        created_at: chrono::Utc::now().to_rfc3339(),
        expires_at: None,
        archived: false,
        favorite: false,
        site: None,
        tags: Vec::new(),
    }
}

/// 进程被外部杀死时，registry 必须把状态改为失败态，
/// 否则 UI 会一直显示「运行中」并挂着一条已失效的死链。
#[tokio::test]
async fn 进程意外退出会被标记为失败态() {
    let Some(child) = spawn_dummy() else {
        eprintln!("跳过：本机没有可用的 node");
        return;
    };
    let pid = child.id().expect("刚 spawn 的进程应有 pid");

    let dir = temp_dir("crash");
    let (registry, _) = TunnelRegistry::with_store(StateStore::in_dir(&dir));
    let registry = Arc::new(registry);

    let inserted = registry.insert(tunnel(19001), child, None).await;
    assert!(matches!(inserted.status, TunnelStatus::Running));

    // 从外部杀掉进程，模拟 cloudflared 自行崩溃。
    #[cfg(windows)]
    let _ = std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    #[cfg(unix)]
    unsafe {
        libc::kill(pid as libc::pid_t, libc::SIGKILL);
    }

    // 监视任务是异步的，给它时间回写状态。
    let mut marked = false;
    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let list = registry.list().await;
        if matches!(list[0].status, TunnelStatus::Failed(_)) {
            assert!(list[0].public_url.is_none(), "失败后必须清空链接");
            marked = true;
            break;
        }
    }
    assert!(marked, "进程已退出但状态未被标记为失败");
    assert_eq!(registry.counts().await.active, 0, "失败的条目不应计入活跃");

    std::fs::remove_dir_all(&dir).ok();
}

/// 用户主动断开时，监视任务不能把它误报成崩溃。
#[tokio::test]
async fn 主动停止不会被误报为崩溃() {
    let Some(child) = spawn_dummy() else {
        eprintln!("跳过：本机没有可用的 node");
        return;
    };

    let dir = temp_dir("stop");
    let (registry, _) = TunnelRegistry::with_store(StateStore::in_dir(&dir));
    let registry = Arc::new(registry);

    let inserted = registry.insert(tunnel(19002), child, None).await;
    registry.stop(&inserted.id).await.unwrap();

    // 给监视任务足够时间；它若误判会把状态改成 Failed。
    tokio::time::sleep(Duration::from_secs(2)).await;

    let list = registry.list().await;
    assert!(
        matches!(list[0].status, TunnelStatus::Stopped),
        "主动停止应保持 Stopped，实际为 {:?}",
        list[0].status
    );
    assert_eq!(registry.counts().await.active, 0);

    std::fs::remove_dir_all(&dir).ok();
}

/// 端口、备注、自动重连开关与累计计数必须跨重启保留，且链接不落盘。
#[tokio::test]
async fn 配置与计数跨重启保留且链接不落盘() {
    let Some(child) = spawn_dummy() else {
        eprintln!("跳过：本机没有可用的 node");
        return;
    };

    let dir = temp_dir("persist");
    let (registry, _) = TunnelRegistry::with_store(StateStore::in_dir(&dir));
    let registry = Arc::new(registry);

    let inserted = registry.insert(tunnel(19003), child, None).await;
    registry.set_auto_start(&inserted.id, true).await.unwrap();
    registry.shutdown_all().await;

    // 落盘文件里不得出现任何公网链接（不变量 6）。
    let raw = std::fs::read_to_string(dir.join("state.json")).unwrap();
    assert!(!raw.contains("trycloudflare"), "链接不得落盘");

    // 模拟重启：用同一目录重新构造 registry。
    let (restored, warning) = TunnelRegistry::with_store(StateStore::in_dir(&dir));
    assert!(warning.is_none());

    let list = restored.list().await;
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].port, 19003);
    assert_eq!(list[0].label.as_deref(), Some("it"));
    assert!(list[0].public_url.is_none(), "重启后不应有链接");
    assert!(matches!(list[0].status, TunnelStatus::Stopped));

    assert_eq!(
        restored.counts().await.total_created,
        1,
        "累计计数应跨重启保留"
    );
    let targets = restored.auto_start_targets().await;
    assert_eq!(targets.len(), 1, "自动重连标记应保留");
    assert_eq!(targets[0].1, 19003);

    std::fs::remove_dir_all(&dir).ok();
}

/// 定时关闭到点后应断开隧道，且状态是「已断开」而非「意外退出」。
#[tokio::test]
async fn 定时关闭到点后断开且不误报为崩溃() {
    let Some(child) = spawn_dummy() else {
        eprintln!("跳过：本机没有可用的 node");
        return;
    };

    let dir = temp_dir("expiry");
    let (registry, _) = TunnelRegistry::with_store(StateStore::in_dir(&dir));
    let registry = Arc::new(registry);

    let inserted = registry.insert(tunnel(19007), child, None).await;

    // 用秒级接口设 1 秒，真实等待定时任务触发——而不是手动调 stop 模拟。
    let deadline = registry
        .set_expiry_secs(&inserted.id, Some(1))
        .await
        .unwrap();
    assert!(deadline.is_some(), "设定定时后应返回到期时刻");
    assert!(
        registry.list().await[0].expires_at.is_some(),
        "expires_at 应反映在快照里"
    );

    // 等定时器自己到点。
    let mut fired = false;
    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if matches!(registry.list().await[0].status, TunnelStatus::Stopped) {
            fired = true;
            break;
        }
    }
    assert!(fired, "定时到点后隧道应被自动断开");

    let t = &registry.list().await[0];
    assert!(
        matches!(t.status, TunnelStatus::Stopped),
        "定时断开应为 Stopped（不能是 Failed），实际 {:?}",
        t.status
    );
    assert!(t.expires_at.is_none(), "断开后应清掉到期时刻");
    assert_eq!(registry.counts().await.active, 0);

    std::fs::remove_dir_all(&dir).ok();
}

/// 取消定时后不应再保留到期时刻。
#[tokio::test]
async fn 取消定时会清掉到期时刻() {
    let Some(child) = spawn_dummy() else {
        eprintln!("跳过：本机没有可用的 node");
        return;
    };

    let dir = temp_dir("expiry-cancel");
    let (registry, _) = TunnelRegistry::with_store(StateStore::in_dir(&dir));
    let registry = Arc::new(registry);

    let inserted = registry.insert(tunnel(19008), child, None).await;
    registry.set_expiry(&inserted.id, Some(10)).await.unwrap();
    assert!(registry.list().await[0].expires_at.is_some());

    let cleared = registry.set_expiry(&inserted.id, None).await.unwrap();

    assert!(cleared.is_none(), "取消时应返回 None");
    assert!(registry.list().await[0].expires_at.is_none());
    // 取消定时不应影响隧道本身。
    assert!(matches!(
        registry.list().await[0].status,
        TunnelStatus::Running
    ));

    registry.shutdown_all().await;
    std::fs::remove_dir_all(&dir).ok();
}

/// 已断开的隧道不能设定定时关闭。
#[tokio::test]
async fn 非活跃隧道不能设定定时() {
    let Some(child) = spawn_dummy() else {
        eprintln!("跳过：本机没有可用的 node");
        return;
    };

    let dir = temp_dir("expiry-inactive");
    let (registry, _) = TunnelRegistry::with_store(StateStore::in_dir(&dir));
    let registry = Arc::new(registry);

    let inserted = registry.insert(tunnel(19009), child, None).await;
    registry.stop(&inserted.id).await.unwrap();

    let result = registry.set_expiry(&inserted.id, Some(5)).await;

    assert!(result.is_err(), "对已断开的隧道设定定时应报错");

    std::fs::remove_dir_all(&dir).ok();
}

/// 归档只影响非活跃条目；归档后记录仍在（历史页可见），不是删除。
#[tokio::test]
async fn 归档保留运行中的映射且不删除记录() {
    let Some(running) = spawn_dummy() else {
        eprintln!("跳过：本机没有可用的 node");
        return;
    };
    let Some(to_stop) = spawn_dummy() else {
        eprintln!("跳过：本机没有可用的 node");
        return;
    };

    let dir = temp_dir("archive");
    let (registry, _) = TunnelRegistry::with_store(StateStore::in_dir(&dir));
    let registry = Arc::new(registry);

    let kept = registry.insert(tunnel(19005), running, None).await;
    let stopped = registry.insert(tunnel(19006), to_stop, None).await;
    registry.stop(&stopped.id).await.unwrap();

    let archived = registry.archive_inactive().await;

    assert_eq!(archived, 1, "只应归档那条已断开的");
    let list = registry.list().await;
    assert_eq!(list.len(), 2, "归档不是删除，两条记录都应还在");

    let kept_item = list.iter().find(|t| t.id == kept.id).unwrap();
    let archived_item = list.iter().find(|t| t.id == stopped.id).unwrap();
    assert!(!kept_item.archived, "运行中的映射不能被归档");
    assert!(archived_item.archived, "已断开的应被归档");

    registry.shutdown_all().await;
    std::fs::remove_dir_all(&dir).ok();
}

/// 活跃映射不允许单独归档：它还在跑，藏起来会让用户失去断开入口。
#[tokio::test]
async fn 活跃映射不能归档() {
    let Some(child) = spawn_dummy() else {
        eprintln!("跳过：本机没有可用的 node");
        return;
    };

    let dir = temp_dir("archive-active");
    let (registry, _) = TunnelRegistry::with_store(StateStore::in_dir(&dir));
    let registry = Arc::new(registry);

    let inserted = registry.insert(tunnel(19010), child, None).await;

    assert!(
        registry.set_archived(&inserted.id, true).await.is_err(),
        "运行中的映射不应允许归档"
    );

    registry.shutdown_all().await;
    std::fs::remove_dir_all(&dir).ok();
}

/// 归档状态要跨重启保留；重新建立映射会自动取消归档（即「从历史恢复」）。
#[tokio::test]
async fn 归档状态跨重启保留且重连会取消归档() {
    let Some(child) = spawn_dummy() else {
        eprintln!("跳过：本机没有可用的 node");
        return;
    };

    let dir = temp_dir("archive-persist");
    let (registry, _) = TunnelRegistry::with_store(StateStore::in_dir(&dir));
    let registry = Arc::new(registry);

    let inserted = registry.insert(tunnel(19011), child, None).await;
    registry.stop(&inserted.id).await.unwrap();
    registry.set_archived(&inserted.id, true).await.unwrap();

    // 重启后归档状态应保留。
    let (restored, _) = TunnelRegistry::with_store(StateStore::in_dir(&dir));
    let restored = Arc::new(restored);
    let list = restored.list().await;
    assert_eq!(list.len(), 1);
    assert!(list[0].archived, "归档状态应跨重启保留");

    // 复用该条目重新建立 = 从历史恢复到映射页，必须取消归档。
    let Some(child2) = spawn_dummy() else {
        return;
    };
    let again = restored
        .insert(tunnel(19011), child2, Some(list[0].id.clone()))
        .await;
    assert!(!again.archived, "重新建立后应回到映射页（取消归档）");

    restored.shutdown_all().await;
    std::fs::remove_dir_all(&dir).ok();
}

/// 移除记录后不应再出现在落盘状态中。
#[tokio::test]
async fn 移除的记录不再落盘() {
    let dir = temp_dir("remove");
    let (registry, _) = TunnelRegistry::with_store(StateStore::in_dir(&dir));
    let registry = Arc::new(registry);

    let Some(child) = spawn_dummy() else {
        eprintln!("跳过：本机没有可用的 node");
        return;
    };
    let inserted = registry.insert(tunnel(19004), child, None).await;
    registry.remove(&inserted.id).await.unwrap();

    let (restored, _) = TunnelRegistry::with_store(StateStore::in_dir(&dir));
    assert!(restored.list().await.is_empty(), "已移除的记录不应被恢复");
    assert_eq!(
        restored.counts().await.total_created,
        1,
        "累计计数是历史总量，不因移除而回退"
    );

    std::fs::remove_dir_all(&dir).ok();
}
