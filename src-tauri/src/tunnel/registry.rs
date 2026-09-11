//! 活跃隧道注册表：进程句柄的唯一持有者，也是计数的唯一来源。
//!
//! 见 AGENTS.md 不变量 5：所有 spawn 必须经由本模块登记，
//! 保证应用退出时能遍历清理，不留孤儿 cloudflared 进程。
//!
//! 进程监视（M4）：`Child` 被包进 `Arc<Mutex<..>>`，registry 与监视任务共享同一句柄。
//! 监视任务 `wait()` 到进程退出后回写状态；用户主动 stop / remove 会先置
//! `expected_exit` 标记，监视任务据此区分「正常断开」与「意外崩溃」。

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::Serialize;
use tokio::process::Child;
use tokio::sync::Mutex;

use super::provider::{SiteInfo, Tunnel, TunnelError, TunnelStatus};
use crate::store::{PersistedState, PersistedTunnel, StateStore};

/// 单条映射的标签数量上限。
///
/// 存在上限只为防止误操作把卡片撑爆（例如粘贴一整段文本）；
/// 正常分类用途下 5 个已经足够。
const MAX_TAGS_PER_TUNNEL: usize = 5;

/// 计数快照，前端只读展示，不在 UI 层维护第二份。
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelCounts {
    /// 当前活跃（Starting + Running）数量。
    pub active: u32,
    /// 历史累计创建成功的数量（跨重启累加，来自落盘状态）。
    pub total_created: u64,
}

/// 正在运行的进程及其「预期退出」标记。
struct Process {
    child: Arc<Mutex<Child>>,
    /// spawn 时记录的 pid。监视任务会持锁 `wait()`，届时无法再从 `Child` 取 pid，
    /// 因此必须在登记时就存下来，供 kill 使用。
    pid: Option<u32>,
    /// 由 stop / remove / shutdown 在杀进程前置位，供监视任务区分退出原因。
    expected_exit: Arc<AtomicBool>,
    /// cloudflared 的本地指标端口，用于查「这条映射被访问了多少次」。
    /// 随进程存亡：进程没了端口也就没了，所以放在 Process 里而不是 Entry。
    metrics_port: Option<u16>,
}

struct Entry {
    tunnel: Tunnel,
    /// Stopped / Failed 的条目保留记录但不再持有进程。
    process: Option<Process>,
    /// 下次启动时是否自动重建。
    auto_start: bool,
    /// 定时关闭任务的句柄。用户提前断开或改期时必须 abort，
    /// 否则旧定时器会在到点时误杀重建后的同 id 隧道。
    expiry_task: Option<tokio::task::AbortHandle>,
}

#[derive(Default)]
struct Inner {
    entries: HashMap<String, Entry>,
    total_created: u64,
    /// Web 控制台的落盘配置。registry 不管理它，只是在重写整个 state.json 时
    /// 原样带上，避免把别人的那一段抹掉。由上层通过 `set_web_snapshot` 更新。
    web: crate::web::console::PersistedWebConsole,
}

impl Inner {
    /// 生成落盘快照。只取可持久化字段，链接与运行态一律不落盘。
    fn snapshot(&self) -> PersistedState {
        let mut tunnels: Vec<(&str, PersistedTunnel)> = self
            .entries
            .values()
            .map(|e| {
                (
                    e.tunnel.created_at.as_str(),
                    PersistedTunnel {
                        port: e.tunnel.port,
                        label: e.tunnel.label.clone(),
                        auto_start: e.auto_start,
                        archived: e.tunnel.archived,
                        favorite: e.tunnel.favorite,
                        site: e.tunnel.site.clone(),
                        tags: e.tunnel.tags.clone(),
                    },
                )
            })
            .collect();
        // HashMap 迭代顺序不稳定，按创建时间排序保证落盘内容可预测。
        tunnels.sort_by(|a, b| a.0.cmp(b.0));

        PersistedState {
            version: 1,
            total_created: self.total_created,
            tunnels: tunnels.into_iter().map(|(_, t)| t).collect(),
            // registry 不拥有这一段，但 save 会重写整个文件——
            // 必须原样带上，否则每次隧道变动都会把 Web 控制台的配置抹掉。
            web: self.web.clone(),
        }
    }
}

#[derive(Default)]
pub struct TunnelRegistry {
    inner: Mutex<Inner>,
    store: StateStore,
}

impl TunnelRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 用落盘状态构造：恢复历史累计计数与隧道条目（条目均为已断开态）。
    ///
    /// 返回读取过程中的告警（若有），由上层转达用户。
    pub fn with_store(store: StateStore) -> (Self, Option<String>) {
        let (state, warning) = store.load();

        let mut entries = HashMap::new();
        for (index, persisted) in state.tunnels.iter().enumerate() {
            let id = uuid::Uuid::new_v4().to_string();
            entries.insert(
                id.clone(),
                Entry {
                    tunnel: Tunnel {
                        id,
                        port: persisted.port,
                        label: persisted.label.clone(),
                        // 链接不可恢复：旧域名随进程退出即失效。
                        public_url: None,
                        status: TunnelStatus::Stopped,
                        // 无原始时间戳，用序号占位保持列表顺序稳定。
                        created_at: format!("restored-{index:04}"),
                        expires_at: None,
                        archived: persisted.archived,
                        favorite: persisted.favorite,
                        // 沿用上次探测结果，让列表启动即有名字可认。
                        // 重新建立映射时会再探测一次，不一致才覆盖。
                        site: persisted.site.clone(),
                        tags: persisted.tags.clone(),
                    },
                    process: None,
                    auto_start: persisted.auto_start,
                    expiry_task: None,
                },
            );
        }

        let registry = Self {
            inner: Mutex::new(Inner {
                entries,
                total_created: state.total_created,
                web: state.web.clone(),
            }),
            store,
        };

        (registry, warning)
    }

    /// 取快照并落盘。调用者必须已释放 `inner` 锁之外的资源；
    /// 本方法内部只在取快照期间持锁，IO 在锁外执行。
    async fn persist(&self) {
        let snapshot = {
            let inner = self.inner.lock().await;
            inner.snapshot()
        };
        if let Err(e) = self.store.save(&snapshot) {
            // 保存失败不影响当前会话可用性，只记录。
            eprintln!("[easy-port] 保存配置失败：{e}");
        }
    }

    /// 端口是否已有活跃隧道。
    pub async fn is_port_mapped(&self, port: u16) -> bool {
        let inner = self.inner.lock().await;
        inner.entries.values().any(|e| {
            e.tunnel.port == port
                && matches!(
                    e.tunnel.status,
                    TunnelStatus::Starting | TunnelStatus::Running
                )
        })
    }

    /// 该端口是否已有记录（含已断开），有则返回其 id。
    ///
    /// 用于复用恢复出来的条目，避免同一端口重连后出现两行。
    pub async fn find_by_port(&self, port: u16) -> Option<String> {
        let inner = self.inner.lock().await;
        inner
            .entries
            .values()
            .find(|e| e.tunnel.port == port)
            .map(|e| e.tunnel.id.clone())
    }

    /// 更新随快照一起落盘的 Web 控制台配置，并立即写盘。
    ///
    /// 存在的原因：`save` 重写整个 state.json，两段配置必须由同一个写入者持有，
    /// 否则后写的一方会覆盖先写的一方。
    pub async fn set_web_snapshot(&self, web: crate::web::console::PersistedWebConsole) {
        {
            let mut inner = self.inner.lock().await;
            inner.web = web;
        }
        self.persist().await;
    }

    /// 读回落盘的 Web 控制台配置，供启动时恢复。
    pub async fn web_snapshot(&self) -> crate::web::console::PersistedWebConsole {
        self.inner.lock().await.web.clone()
    }

    /// 同步版本，仅供 Tauri 的 setup 钩子使用。
    ///
    /// setup 不是 async，而此刻 registry 刚构造、没有任何并发访问者，
    /// `try_lock` 必然成功。拿不到锁说明调用时机错了，用默认值兜底而不是 panic。
    pub fn web_snapshot_blocking(&self) -> crate::web::console::PersistedWebConsole {
        self.inner
            .try_lock()
            .map(|inner| inner.web.clone())
            .unwrap_or_default()
    }

    /// 按 id 取端口与备注。供 Web 控制台「开启已有映射」使用：
    /// 它只允许操作**已存在**的条目，不能凭端口号凭空新建。
    pub async fn find_port_by_id(&self, id: &str) -> Option<(u16, Option<String>)> {
        let inner = self.inner.lock().await;
        inner
            .entries
            .get(id)
            .map(|e| (e.tunnel.port, e.tunnel.label.clone()))
    }

    /// 取所有活跃条目的指标端口。
    ///
    /// 只返回端口不返回指标本身：抓取要发 HTTP 请求，不能占着注册表的锁做——
    /// 那会让整个列表在抓取期间卡住。调用方拿到端口后自行并发抓。
    pub async fn metrics_ports(&self) -> Vec<(String, u16)> {
        let inner = self.inner.lock().await;
        inner
            .entries
            .iter()
            .filter_map(|(id, e)| {
                let port = e.process.as_ref()?.metrics_port?;
                Some((id.clone(), port))
            })
            .collect()
    }

    /// 登记一条已成功建立的隧道，并启动进程监视任务。
    ///
    /// `existing_id` 为 `Some` 时原地复用该条目（保留其 auto_start 与列表位置）。
    pub async fn insert(
        self: &Arc<Self>,
        mut tunnel: Tunnel,
        child: Child,
        existing_id: Option<String>,
        metrics_port: Option<u16>,
    ) -> Tunnel {
        // pid 必须在交出所有权前取出：之后监视任务会持锁 wait，取不到了。
        let pid = child.id();
        let child = Arc::new(Mutex::new(child));
        let expected_exit = Arc::new(AtomicBool::new(false));

        {
            let mut inner = self.inner.lock().await;
            inner.total_created += 1;

            let process = Some(Process {
                child: Arc::clone(&child),
                pid,
                expected_exit: Arc::clone(&expected_exit),
                metrics_port,
            });

            match existing_id.and_then(|id| inner.entries.remove(&id).map(|e| (id, e))) {
                // 复用旧条目：沿用其 id、创建时间与自动重连设置。
                Some((id, old)) => {
                    tunnel.id = id.clone();
                    tunnel.created_at = old.tunnel.created_at;
                    // 重新建立即视为「恢复到映射页」，必须取消归档，
                    // 否则从历史恢复的映射会跑起来却看不见。
                    tunnel.archived = false;
                    // 收藏是用户意图，重连不该把它清掉。
                    tunnel.favorite = old.tunnel.favorite;
                    // 先顶上旧的站点信息，避免重连瞬间标题闪没；
                    // 随后的探测拿到新结果不一致时会覆盖它。
                    tunnel.site = old.tunnel.site;
                    // 标签同收藏，是用户意图，重连不该把它清掉。
                    tunnel.tags = old.tunnel.tags;
                    // 复用条目时旧定时器必须作废，否则会在到点时误杀这条新隧道。
                    if let Some(task) = old.expiry_task {
                        task.abort();
                    }
                    inner.entries.insert(
                        id,
                        Entry {
                            tunnel: tunnel.clone(),
                            process,
                            auto_start: old.auto_start,
                            expiry_task: None,
                        },
                    );
                }
                None => {
                    inner.entries.insert(
                        tunnel.id.clone(),
                        Entry {
                            tunnel: tunnel.clone(),
                            process,
                            auto_start: false,
                            expiry_task: None,
                        },
                    );
                }
            }
        }

        self.watch_process(tunnel.id.clone(), child, expected_exit);
        // 站点信息异步探测，不拖慢建立流程。
        self.spawn_site_probe(tunnel.id.clone(), tunnel.port);
        self.persist().await;
        tunnel
    }

    /// 监视子进程退出。非预期退出时把条目标为失败态，避免 UI 上留下「运行中」的死链。
    fn watch_process(
        self: &Arc<Self>,
        id: String,
        child: Arc<Mutex<Child>>,
        expected_exit: Arc<AtomicBool>,
    ) {
        let registry = Arc::clone(self);
        tokio::spawn(async move {
            // 持锁 await `wait()`：stop 路径改用 kill by pid，不争这把锁，
            // 否则 kill 会被 wait 永久阻塞。
            let status = {
                let mut guard = child.lock().await;
                guard.wait().await
            };

            if expected_exit.load(Ordering::SeqCst) {
                return; // 用户主动断开，状态已由 stop/remove 写好。
            }

            let reason = match status {
                Ok(s) => format!("隧道进程意外退出（{s}）"),
                Err(e) => format!("隧道进程状态未知：{e}"),
            };

            let mut inner = registry.inner.lock().await;
            if let Some(entry) = inner.entries.get_mut(&id) {
                // 只覆盖仍被认为活跃的条目，不回退已被主动改写的状态。
                if matches!(
                    entry.tunnel.status,
                    TunnelStatus::Starting | TunnelStatus::Running
                ) {
                    entry.tunnel.status = TunnelStatus::Failed(reason);
                    entry.tunnel.public_url = None;
                    entry.tunnel.expires_at = None;
                    entry.process = None;
                    // 进程已死，定时器再触发也只是空转，直接作废。
                    if let Some(task) = entry.expiry_task.take() {
                        task.abort();
                    }
                }
            }
        });
    }

    /// 停止指定隧道并杀掉其子进程。
    pub async fn stop(&self, id: &str) -> Result<(), TunnelError> {
        {
            let mut inner = self.inner.lock().await;
            let entry = inner
                .entries
                .get_mut(id)
                .ok_or(TunnelError::TunnelNotFound)?;

            if let Some(process) = entry.process.take() {
                kill(&process).await;
            }
            // 定时器已无意义（可能就是它触发的本次 stop，abort 自身是安全的）。
            if let Some(task) = entry.expiry_task.take() {
                task.abort();
            }
            entry.tunnel.status = TunnelStatus::Stopped;
            entry.tunnel.public_url = None;
            entry.tunnel.expires_at = None;
        }
        self.persist().await;
        Ok(())
    }

    /// 移除一条记录。
    pub async fn remove(&self, id: &str) -> Result<(), TunnelError> {
        {
            let mut inner = self.inner.lock().await;
            let mut entry = inner.entries.remove(id).ok_or(TunnelError::TunnelNotFound)?;
            if let Some(process) = entry.process.take() {
                kill(&process).await;
            }
            if let Some(task) = entry.expiry_task.take() {
                task.abort();
            }
        }
        self.persist().await;
        Ok(())
    }

    /// 设置某条隧道是否在下次启动时自动重建。
    pub async fn set_auto_start(&self, id: &str, enabled: bool) -> Result<(), TunnelError> {
        {
            let mut inner = self.inner.lock().await;
            let entry = inner
                .entries
                .get_mut(id)
                .ok_or(TunnelError::TunnelNotFound)?;
            entry.auto_start = enabled;
        }
        self.persist().await;
        Ok(())
    }

    /// 列出需要自动重建的条目：`(id, port, label)`。
    ///
    /// 只返回当前非活跃的条目，避免重复建立。
    pub async fn auto_start_targets(&self) -> Vec<(String, u16, Option<String>)> {
        let inner = self.inner.lock().await;
        let mut items: Vec<_> = inner
            .entries
            .values()
            .filter(|e| {
                e.auto_start
                    && !matches!(
                        e.tunnel.status,
                        TunnelStatus::Starting | TunnelStatus::Running
                    )
            })
            .map(|e| {
                (
                    e.tunnel.created_at.clone(),
                    e.tunnel.id.clone(),
                    e.tunnel.port,
                    e.tunnel.label.clone(),
                )
            })
            .collect();
        items.sort_by(|a, b| a.0.cmp(&b.0));
        items.into_iter().map(|(_, id, p, l)| (id, p, l)).collect()
    }

    /// 为指定隧道设定定时关闭；`minutes` 为 None 时取消已有定时。
    ///
    /// 只能对活跃隧道设定。到期后走与用户主动断开相同的 `stop` 路径，
    /// 因此同样会置 `expected_exit`，不会被监视任务误报为崩溃。
    pub async fn set_expiry(
        self: &Arc<Self>,
        id: &str,
        minutes: Option<u32>,
    ) -> Result<Option<String>, TunnelError> {
        self.set_expiry_secs(id, minutes.map(|m| m as u64 * 60)).await
    }

    /// 以秒为单位设定定时关闭。`set_expiry` 的实现，同时供测试用短时限验证真实触发。
    pub async fn set_expiry_secs(
        self: &Arc<Self>,
        id: &str,
        seconds: Option<u64>,
    ) -> Result<Option<String>, TunnelError> {
        let mut inner = self.inner.lock().await;
        let entry = inner
            .entries
            .get_mut(id)
            .ok_or(TunnelError::TunnelNotFound)?;

        if !matches!(
            entry.tunnel.status,
            TunnelStatus::Starting | TunnelStatus::Running
        ) {
            return Err(TunnelError::TunnelNotActive);
        }

        // 改期或取消前先废掉旧定时器，避免两个定时器同时存在。
        if let Some(task) = entry.expiry_task.take() {
            task.abort();
        }

        let Some(seconds) = seconds.filter(|s| *s > 0) else {
            entry.tunnel.expires_at = None;
            return Ok(None);
        };

        let deadline = chrono::Utc::now() + chrono::Duration::seconds(seconds as i64);
        let deadline_text = deadline.to_rfc3339();
        entry.tunnel.expires_at = Some(deadline_text.clone());

        let registry = Arc::clone(self);
        let id_owned = id.to_string();
        let handle = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(seconds)).await;
            // 走正常 stop 路径：杀进程、置状态、落盘一次完成。
            let _ = registry.stop(&id_owned).await;
        });
        entry.expiry_task = Some(handle.abort_handle());

        Ok(Some(deadline_text))
    }

    /// 设置备注。空串按「清除备注」处理，存为 `None` 而不是 `Some("")`。
    ///
    /// 与运行状态无关：备注属于「这个端口是干什么的」这类长期信息，
    /// 已断开、已归档的条目同样可以改。
    pub async fn set_label(&self, id: &str, label: Option<String>) -> Result<(), TunnelError> {
        {
            let mut inner = self.inner.lock().await;
            let entry = inner
                .entries
                .get_mut(id)
                .ok_or(TunnelError::TunnelNotFound)?;
            // 前端传空串与传 null 语义相同，统一收敛为 None，
            // 否则落盘里会出现 "label": "" 这种既非空又无内容的值。
            entry.tunnel.label = label
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
        }
        self.persist().await;
        Ok(())
    }

    /// 覆盖式设置标签。
    ///
    /// 逐个裁空白、去重（大小写不敏感，保留首次出现的写法）、丢弃空串。
    /// 大小写不敏感是因为「Dev」和「dev」在用户眼里是同一个分类，
    /// 允许两者共存会让筛选栏出现看起来重复的标签。
    pub async fn set_tags(&self, id: &str, tags: Vec<String>) -> Result<(), TunnelError> {
        let mut seen = std::collections::HashSet::new();
        let cleaned: Vec<String> = tags
            .into_iter()
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .filter(|t| seen.insert(t.to_lowercase()))
            .take(MAX_TAGS_PER_TUNNEL)
            .collect();

        {
            let mut inner = self.inner.lock().await;
            let entry = inner
                .entries
                .get_mut(id)
                .ok_or(TunnelError::TunnelNotFound)?;
            entry.tunnel.tags = cleaned;
        }
        self.persist().await;
        Ok(())
    }

    /// 当前用到的全部标签，按名称排序。
    ///
    /// 「随用随建」模式下没有独立的标签表——标签的存在性完全由「是否有端口在用」
    /// 决定，因此这里从条目里现算。最后一个端口移除某标签后，它自然从列表消失。
    pub async fn all_tags(&self) -> Vec<String> {
        let inner = self.inner.lock().await;
        let mut seen = std::collections::HashSet::new();
        let mut tags: Vec<String> = inner
            .entries
            .values()
            .flat_map(|e| e.tunnel.tags.iter().cloned())
            .filter(|t| seen.insert(t.to_lowercase()))
            .collect();
        tags.sort_by_key(|t| t.to_lowercase());
        tags
    }

    /// 设置收藏状态。收藏与运行状态无关，已断开的也能收藏。
    pub async fn set_favorite(&self, id: &str, favorite: bool) -> Result<(), TunnelError> {
        {
            let mut inner = self.inner.lock().await;
            let entry = inner
                .entries
                .get_mut(id)
                .ok_or(TunnelError::TunnelNotFound)?;
            entry.tunnel.favorite = favorite;
        }
        self.persist().await;
        Ok(())
    }

    /// 写入探测到的站点信息，变化时落盘。
    ///
    /// 条目已不存在时静默忽略（探测是异步的，可能已被删除）。
    /// 返回是否真的发生了变化——相同结果不重复写盘，避免每次重连都改写 state.json。
    pub async fn set_site(&self, id: &str, site: Option<SiteInfo>) -> bool {
        let changed = {
            let mut inner = self.inner.lock().await;
            match inner.entries.get_mut(id) {
                Some(entry) if entry.tunnel.site != site => {
                    entry.tunnel.site = site;
                    true
                }
                _ => false,
            }
        };
        if changed {
            self.persist().await;
        }
        changed
    }

    /// 后台探测本机服务的标题与图标，完成后写回条目。
    ///
    /// 不阻塞建立流程：探测失败或超时都只是没有标题，映射本身照常可用。
    ///
    /// **探测失败时保留旧值**：目标服务可能只是还没起来或响应慢，
    /// 此时把上次的标题清成空白，用户看到的是列表退化成一排端口号，反而更糟。
    /// 只有拿到新结果且与旧值不同才覆盖。
    pub fn spawn_site_probe(self: &Arc<Self>, id: String, port: u16) {
        let registry = Arc::clone(self);
        tokio::spawn(async move {
            if let Some(site) = super::site::probe(port).await {
                registry.set_site(&id, Some(site)).await;
            }
        });
    }

    /// 设置单条记录的归档状态。
    ///
    /// 归档 = 从「映射」页隐藏，但记录仍在（「历史」页可见、可恢复）。
    /// 活跃的映射不允许归档：它还在跑，藏起来会让用户失去断开入口。
    pub async fn set_archived(&self, id: &str, archived: bool) -> Result<(), TunnelError> {
        {
            let mut inner = self.inner.lock().await;
            let entry = inner
                .entries
                .get_mut(id)
                .ok_or(TunnelError::TunnelNotFound)?;

            if archived
                && matches!(
                    entry.tunnel.status,
                    TunnelStatus::Starting | TunnelStatus::Running
                )
            {
                return Err(TunnelError::TunnelStillActive);
            }
            entry.tunnel.archived = archived;
        }
        self.persist().await;
        Ok(())
    }

    /// 归档所有非活跃条目，返回归档条数。
    ///
    /// 与删除的区别：记录仍保留在「历史」页，只是不再出现在「映射」页。
    pub async fn archive_inactive(&self) -> usize {
        let count = {
            let mut inner = self.inner.lock().await;
            let mut count = 0;
            for entry in inner.entries.values_mut() {
                let inactive = !matches!(
                    entry.tunnel.status,
                    TunnelStatus::Starting | TunnelStatus::Running
                );
                if inactive && !entry.tunnel.archived {
                    entry.tunnel.archived = true;
                    count += 1;
                }
            }
            count
        };
        if count > 0 {
            self.persist().await;
        }
        count
    }

    /// 彻底删除所有已归档的记录，返回删除条数。供「历史」页清空使用。
    pub async fn purge_archived(&self) -> usize {
        let removed = {
            let mut inner = self.inner.lock().await;
            let ids: Vec<String> = inner
                .entries
                .values()
                .filter(|e| e.tunnel.archived)
                .map(|e| e.tunnel.id.clone())
                .collect();
            for id in &ids {
                inner.entries.remove(id);
            }
            ids.len()
        };
        if removed > 0 {
            self.persist().await;
        }
        removed
    }

    /// 启动时校验落盘的站点信息是否仍然准确。
    ///
    /// 对每条存有站点信息的条目重新探测本机端口：拿到不同的结果就更新，
    /// 探测不到（服务没起、端口换了用途）则**保留旧值**——
    /// 宁可显示一个可能过时的名字，也好过让列表退化成一排端口号。
    ///
    /// 与自动重连相互独立：没开自动重连的条目也会被校验，
    /// 否则它们的标题会一直停在第一次探测的样子。
    pub fn spawn_site_refresh(self: &Arc<Self>) {
        let registry = Arc::clone(self);
        tokio::spawn(async move {
            let targets: Vec<(String, u16)> = {
                let inner = registry.inner.lock().await;
                inner
                    .entries
                    .values()
                    .filter(|e| e.tunnel.site.is_some())
                    .map(|e| (e.tunnel.id.clone(), e.tunnel.port))
                    .collect()
            };

            for (id, port) in targets {
                if let Some(site) = super::site::probe(port).await {
                    registry.set_site(&id, Some(site)).await;
                }
            }
        });
    }

    /// 把某条隧道标记为失败态（用于恢复失败等场景）。
    pub async fn mark_failed(&self, id: &str, reason: String) {
        let mut inner = self.inner.lock().await;
        if let Some(entry) = inner.entries.get_mut(id) {
            entry.tunnel.status = TunnelStatus::Failed(reason);
            entry.tunnel.public_url = None;
            entry.tunnel.expires_at = None;
            entry.process = None;
            if let Some(task) = entry.expiry_task.take() {
                task.abort();
            }
        }
    }

    /// 列出全部隧道，按创建时间倒序。
    pub async fn list(&self) -> Vec<Tunnel> {
        let inner = self.inner.lock().await;
        let mut items: Vec<Tunnel> = inner.entries.values().map(|e| e.tunnel.clone()).collect();
        items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        items
    }

    /// 某条隧道是否开启了自动重连，供前端渲染开关。
    pub async fn auto_start_flags(&self) -> HashMap<String, bool> {
        let inner = self.inner.lock().await;
        inner
            .entries
            .values()
            .map(|e| (e.tunnel.id.clone(), e.auto_start))
            .collect()
    }

    /// 当前计数快照。
    pub async fn counts(&self) -> TunnelCounts {
        let inner = self.inner.lock().await;
        let active = inner
            .entries
            .values()
            .filter(|e| {
                matches!(
                    e.tunnel.status,
                    TunnelStatus::Starting | TunnelStatus::Running
                )
            })
            .count() as u32;
        TunnelCounts {
            active,
            total_created: inner.total_created,
        }
    }

    /// 应用退出时的兜底清理：杀掉所有仍存活的子进程。
    pub async fn shutdown_all(&self) {
        // 退出前先落盘，保证最后一次配置变更不丢。
        self.persist().await;

        let mut inner = self.inner.lock().await;
        for entry in inner.entries.values_mut() {
            if let Some(process) = entry.process.take() {
                kill(&process).await;
            }
            if let Some(task) = entry.expiry_task.take() {
                task.abort();
            }
            entry.tunnel.status = TunnelStatus::Stopped;
            entry.tunnel.public_url = None;
            entry.tunnel.expires_at = None;
        }
    }
}

/// 杀掉子进程，并先置预期退出标记，避免监视任务误报为崩溃。
///
/// 监视任务持有 `child` 的锁在 `wait()`，这里抢同一把锁会死等，
/// 因此优先走 `try_lock`；拿不到锁时用登记时存下的 pid 发系统终止信号。
async fn kill(process: &Process) {
    process.expected_exit.store(true, Ordering::SeqCst);

    // 没有监视任务在 wait 时（如单元测试）可直接走 tokio 的 kill。
    if let Ok(mut child) = process.child.try_lock() {
        let _ = child.start_kill();
        return;
    }

    if let Some(pid) = process.pid {
        kill_pid(pid).await;
    }
}

/// 按 pid 终止进程。监视任务的 `wait()` 会随之返回并回收僵尸进程。
///
/// `pub(crate)`：Web 控制台的隧道不进 registry，但终止方式必须一致。
#[cfg(windows)]
pub(crate) async fn kill_pid(pid: u32) {
    use std::process::Stdio;
    // /T 连同子进程一并终止，避免 cloudflared 派生的进程残留。
    let _ = tokio::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW，不弹控制台
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;
}

#[cfg(unix)]
pub(crate) async fn kill_pid(pid: u32) {
    // SAFETY: kill(2) 对已退出的 pid 只会返回 ESRCH，不会有内存安全影响。
    unsafe {
        libc::kill(pid as libc::pid_t, libc::SIGKILL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tunnel(port: u16, created_at: &str) -> Tunnel {
        Tunnel {
            id: uuid::Uuid::new_v4().to_string(),
            port,
            label: None,
            public_url: Some("https://example.trycloudflare.com".into()),
            status: TunnelStatus::Running,
            created_at: created_at.into(),
            expires_at: None,
            archived: false,
            favorite: false,
            site: None,
            tags: Vec::new(),
        }
    }

    #[tokio::test]
    async fn 落盘状态能恢复计数与条目() {
        let dir = std::env::temp_dir().join(format!("easy-port-reg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = StateStore::in_dir(&dir);
        store
            .save(&PersistedState {
                version: 1,
                total_created: 12,
                web: Default::default(),
            tunnels: vec![PersistedTunnel {
                    port: 3000,
                    label: Some("dev".into()),
                    auto_start: true,
                    archived: false,
                    favorite: false,
                    site: Some(SiteInfo {
                        title: Some("上次看到的标题".into()),
                        icon: None,
                    }),
                    tags: Vec::new(),
                }],
            })
            .unwrap();

        let (registry, warning) = TunnelRegistry::with_store(StateStore::in_dir(&dir));

        assert!(warning.is_none());
        let counts = registry.counts().await;
        assert_eq!(counts.total_created, 12, "历史累计应跨重启保留");
        assert_eq!(counts.active, 0, "恢复出来的条目不算活跃");

        let list = registry.list().await;
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].port, 3000);
        assert!(list[0].public_url.is_none(), "链接不可恢复");
        // 站点信息与链接不同：它可恢复，启动即有名字可认，之后再探测校正。
        assert_eq!(
            list[0].site.as_ref().and_then(|s| s.title.as_deref()),
            Some("上次看到的标题"),
            "站点标题应沿用上次探测结果"
        );

        let targets = registry.auto_start_targets().await;
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].1, 3000);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn 隧道变动不会抹掉_web_控制台配置() {
        let dir = std::env::temp_dir().join(format!("easy-port-reg-webcfg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let registry = Arc::new(TunnelRegistry::with_store(StateStore::in_dir(&dir)).0);

        // 先存一份控制台配置
        registry
            .set_web_snapshot(crate::web::console::PersistedWebConsole {
                port: Some(18888),
                label: Some("远程".into()),
                token_hash: Some("$argon2id$v=19$x".into()),
                auto_start: false,
            })
            .await;

        // 再触发一次隧道侧的落盘。save 会重写整个文件，
        // 如果 snapshot 不带上 web 段，这里就会把它抹掉。
        {
            let mut inner = registry.inner.lock().await;
            let t = tunnel(3000, "2026-01-01T00:00:00Z");
            inner.entries.insert(
                t.id.clone(),
                Entry {
                    tunnel: t,
                    process: None,
                    auto_start: false,
                    expiry_task: None,
                },
            );
        }
        registry.persist().await;

        let (restored, _) = TunnelRegistry::with_store(StateStore::in_dir(&dir));
        let web = restored.web_snapshot().await;
        assert_eq!(web.port, Some(18888), "控制台端口不该被隧道落盘冲掉");
        assert_eq!(web.label.as_deref(), Some("远程"));
        assert!(web.token_hash.is_some(), "token 哈希不该丢");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn 标签去重裁空白且可跨端口聚合() {
        let dir = std::env::temp_dir().join(format!("easy-port-reg-tags-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let (registry, _) = TunnelRegistry::with_store(StateStore::in_dir(&dir));
        let mut ids = Vec::new();
        for (i, port) in [3000u16, 5173].iter().enumerate() {
            let mut inner = registry.inner.lock().await;
            let t = tunnel(*port, &format!("2026-01-0{}T00:00:00Z", i + 1));
            ids.push(t.id.clone());
            inner.entries.insert(
                t.id.clone(),
                Entry {
                    tunnel: t,
                    process: None,
                    auto_start: false,
                    expiry_task: None,
                },
            );
        }

        // 裁空白、丢空串、大小写不敏感去重（保留首次出现的写法）
        registry
            .set_tags(
                &ids[0],
                vec![
                    "  前端  ".into(),
                    "".into(),
                    "Dev".into(),
                    "dev".into(),
                    "   ".into(),
                ],
            )
            .await
            .unwrap();
        let list = registry.list().await;
        let first = list.iter().find(|t| t.id == ids[0]).unwrap();
        assert_eq!(first.tags, vec!["前端".to_string(), "Dev".to_string()]);

        // 另一个端口复用同名标签（写法不同），聚合时应只出现一次
        registry
            .set_tags(&ids[1], vec!["dev".into(), "后端".into()])
            .await
            .unwrap();
        let all = registry.all_tags().await;
        assert_eq!(all.len(), 3, "Dev/dev 应聚合为一个，实际：{all:?}");
        assert!(all.contains(&"后端".to_string()));

        // 超出上限的部分被截断，不至于把卡片撑爆
        registry
            .set_tags(
                &ids[0],
                (1..=10).map(|i| format!("标签{i}")).collect(),
            )
            .await
            .unwrap();
        let list = registry.list().await;
        let first = list.iter().find(|t| t.id == ids[0]).unwrap();
        assert_eq!(first.tags.len(), MAX_TAGS_PER_TUNNEL);

        // 跨重启保留：此时 3000 有 5 个「标签N」，5173 仍有 dev + 后端
        let (restored, _) = TunnelRegistry::with_store(StateStore::in_dir(&dir));
        assert_eq!(restored.all_tags().await.len(), MAX_TAGS_PER_TUNNEL + 2);

        // 清空后该标签从聚合列表消失——「随用随建」没有独立标签表
        registry.set_tags(&ids[0], Vec::new()).await.unwrap();
        registry.set_tags(&ids[1], Vec::new()).await.unwrap();
        assert!(registry.all_tags().await.is_empty());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn 备注可随时修改且空串收敛为无备注() {
        let dir = std::env::temp_dir().join(format!("easy-port-reg-label-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let (registry, _) = TunnelRegistry::with_store(StateStore::in_dir(&dir));
        let id = {
            let mut inner = registry.inner.lock().await;
            let t = tunnel(3000, "2026-01-01T00:00:00Z");
            let id = t.id.clone();
            inner.entries.insert(
                id.clone(),
                Entry {
                    tunnel: t,
                    process: None,
                    auto_start: false,
                    expiry_task: None,
                },
            );
            id
        };

        // 已断开 / 未运行的条目同样可以改备注
        registry
            .set_label(&id, Some("  开发服务器  ".into()))
            .await
            .unwrap();
        let list = registry.list().await;
        assert_eq!(
            list[0].label.as_deref(),
            Some("开发服务器"),
            "首尾空白应被裁掉"
        );

        // 跨重启保留
        let (restored, _) = TunnelRegistry::with_store(StateStore::in_dir(&dir));
        assert_eq!(restored.list().await[0].label.as_deref(), Some("开发服务器"));

        // 空串等同于清除，不该留下 Some("")
        registry.set_label(&id, Some("   ".into())).await.unwrap();
        assert!(
            registry.list().await[0].label.is_none(),
            "全空白应收敛为无备注"
        );

        registry.set_label("不存在的-id", None).await.unwrap_err();

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn 站点信息变化才落盘且能跨重启保留() {
        let dir = std::env::temp_dir().join(format!("easy-port-reg-site-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let (registry, _) = TunnelRegistry::with_store(StateStore::in_dir(&dir));
        let id = {
            let mut inner = registry.inner.lock().await;
            let t = tunnel(5173, "2026-01-01T00:00:00Z");
            let id = t.id.clone();
            inner.entries.insert(
                id.clone(),
                Entry {
                    tunnel: t,
                    process: None,
                    auto_start: false,
                    expiry_task: None,
                },
            );
            id
        };

        let first = SiteInfo {
            title: Some("旧标题".into()),
            icon: None,
        };
        assert!(
            registry.set_site(&id, Some(first.clone())).await,
            "首次写入应视为变化"
        );
        assert!(
            !registry.set_site(&id, Some(first)).await,
            "相同结果不应重复落盘"
        );

        // 端口上换了服务：新标题必须覆盖旧的。
        assert!(
            registry
                .set_site(
                    &id,
                    Some(SiteInfo {
                        title: Some("新标题".into()),
                        icon: None,
                    })
                )
                .await,
            "不一致时应更新"
        );

        // 重启后仍是新标题。
        let (restored, _) = TunnelRegistry::with_store(StateStore::in_dir(&dir));
        let list = restored.list().await;
        assert_eq!(
            list[0].site.as_ref().and_then(|s| s.title.as_deref()),
            Some("新标题")
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn 设置自动重连会落盘() {
        let dir = std::env::temp_dir().join(format!("easy-port-reg-auto-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let (registry, _) = TunnelRegistry::with_store(StateStore::in_dir(&dir));
        {
            let mut inner = registry.inner.lock().await;
            let t = tunnel(8080, "2026-01-01T00:00:00Z");
            inner.entries.insert(
                t.id.clone(),
                Entry {
                    tunnel: t,
                    process: None,
                    auto_start: false,
                    expiry_task: None,
                },
            );
        }
        let id = registry.list().await[0].id.clone();

        registry.set_auto_start(&id, true).await.unwrap();

        let (restored, _) = TunnelRegistry::with_store(StateStore::in_dir(&dir));
        assert_eq!(restored.auto_start_targets().await.len(), 1);

        std::fs::remove_dir_all(&dir).ok();
    }
}
