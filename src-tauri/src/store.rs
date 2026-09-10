//! 隧道配置与计数的落盘层。
//!
//! 见 AGENTS.md 不变量 3：持久化数据量极小，用单个 JSON 文件，不引入数据库。
//!
//! 关键设计（勿改成缓存 URL）：**公网链接不落盘**。
//! cloudflared Quick Tunnel 的域名由边缘在进程存活期间临时分配，进程退出即失效且
//! 不可复用，缓存下来只会得到一条打不开的死链；同时不变量 6 要求链接不留存。
//! 因此这里只存用户**意图**（端口 / 备注 / 是否自动重连 / 收藏 / 归档），恢复时重新 spawn 拿新链接。
//!
//! 唯一的例外是 `site`（站点标题与图标）：它落盘只为让列表启动即有名字可认，
//! 且每次启动都会重新探测校验、不一致就覆盖，因此不属于「缓存失效数据」。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::tunnel::provider::SiteInfo;

/// 落盘格式版本号，预留迁移位。
const STATE_VERSION: u32 = 1;

const STATE_FILE: &str = "state.json";

/// 单条隧道的可持久化部分。
///
/// 刻意不含 `id`（每次运行重新生成）、`public_url`、`status`（都是运行态）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistedTunnel {
    pub port: u16,
    #[serde(default)]
    pub label: Option<String>,
    /// 下次启动时是否自动重新建立。
    #[serde(default)]
    pub auto_start: bool,
    /// 是否已归档。归档后从「映射」页隐藏，但仍保留在「历史」页。
    ///
    /// `serde(default)` 保证旧版本的 state.json 仍可读（缺该字段视为未归档）。
    #[serde(default)]
    pub archived: bool,
    /// 是否收藏。收藏的映射在「映射」页置顶成独立分区。
    #[serde(default)]
    pub favorite: bool,
    /// 上次探测到的站点标题与图标。
    ///
    /// 落盘的目的是**让列表启动即有名字可认**，而不是长期缓存：
    /// 每次重新建立映射都会再探测一次，结果不同就覆盖（见 `registry::spawn_site_probe`）。
    /// 因此这里存的是「上一次看到的样子」，在新结果到达前先顶上，避免列表退化成一排端口号。
    #[serde(default)]
    pub site: Option<SiteInfo>,
    /// 人工分类标签。与 `label`（备注）不同，可有多个且跨端口复用。
    #[serde(default)]
    pub tags: Vec<String>,
}

/// 落盘状态整体。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistedState {
    pub version: u32,
    /// 历史累计创建成功数量（跨重启累加）。
    #[serde(default)]
    pub total_created: u64,
    #[serde(default)]
    pub tunnels: Vec<PersistedTunnel>,
    /// Web 控制台的配置。`serde(default)` 保证旧文件缺这一段也能读。
    #[serde(default)]
    pub web: crate::web::console::PersistedWebConsole,
}

impl Default for PersistedState {
    fn default() -> Self {
        Self {
            version: STATE_VERSION,
            total_created: 0,
            tunnels: Vec::new(),
            web: Default::default(),
        }
    }
}

/// 状态文件的读写器。
///
/// `path` 为 `None` 时（无法确定 app data 目录）降级为纯内存运行：
/// 读返回默认值，写静默跳过，不阻断应用可用性。
#[derive(Debug, Default)]
pub struct StateStore {
    path: Option<PathBuf>,
}

impl StateStore {
    pub fn new(path: Option<PathBuf>) -> Self {
        Self { path }
    }

    /// 基于 app data 目录构造。
    pub fn in_dir(dir: impl AsRef<Path>) -> Self {
        Self::new(Some(dir.as_ref().join(STATE_FILE)))
    }

    /// 读取落盘状态。
    ///
    /// 任何失败（文件不存在 / JSON 损坏 / 版本不认识）都退回默认值，
    /// 只在返回值里附带一条面向用户的告警，绝不阻断启动。
    pub fn load(&self) -> (PersistedState, Option<String>) {
        let Some(path) = &self.path else {
            return (
                PersistedState::default(),
                Some("无法定位配置目录，本次运行不会保存隧道配置".into()),
            );
        };

        let raw = match std::fs::read_to_string(path) {
            Ok(raw) => raw,
            // 首次运行没有文件属正常情况，不告警。
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return (PersistedState::default(), None);
            }
            Err(e) => {
                return (
                    PersistedState::default(),
                    Some(format!("读取配置失败，已从空列表启动：{e}")),
                );
            }
        };

        match serde_json::from_str::<PersistedState>(&raw) {
            Ok(state) if state.version == STATE_VERSION => (state, None),
            // 版本不认识时不猜测字段含义，保守退回默认值。
            Ok(state) => (
                PersistedState::default(),
                Some(format!(
                    "配置版本 {} 不受支持（当前 {STATE_VERSION}），已从空列表启动",
                    state.version
                )),
            ),
            Err(e) => (
                PersistedState::default(),
                Some(format!("配置文件已损坏，已从空列表启动：{e}")),
            ),
        }
    }

    /// 原子写入：先写临时文件再 rename，避免中途失败留下半个 JSON。
    pub fn save(&self, state: &PersistedState) -> std::io::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(state)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json)?;
        // Windows 下 rename 到已存在的文件会失败，用 std::fs::rename 前先删旧文件。
        // 这一步失败不影响 tmp 的完整性，下次保存会重试。
        if path.exists() {
            let _ = std::fs::remove_file(path);
        }
        std::fs::rename(&tmp, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 用进程 id + 计数器造一个独立临时目录，避免测试并发互相干扰。
    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "easy-port-test-{}-{}-{tag}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn 首次运行无文件时返回默认值且不告警() {
        let dir = temp_dir("fresh");
        let store = StateStore::in_dir(&dir);

        let (state, warning) = store.load();

        assert_eq!(state.version, STATE_VERSION);
        assert!(state.tunnels.is_empty());
        assert_eq!(state.total_created, 0);
        assert!(warning.is_none(), "首次运行不应告警");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 保存后能原样读回() {
        let dir = temp_dir("roundtrip");
        let store = StateStore::in_dir(&dir);

        let state = PersistedState {
            version: STATE_VERSION,
            total_created: 7,
            web: Default::default(),
            tunnels: vec![
                PersistedTunnel {
                    port: 3000,
                    label: Some("dev".into()),
                    auto_start: true,
                    archived: false,
                    favorite: false,
                    site: Some(SiteInfo {
                        title: Some("我的开发服务器".into()),
                        icon: Some("data:image/png;base64,iVBORw0KGgo=".into()),
                    }),
                    tags: vec!["前端".into(), "常用".into()],
                },
                PersistedTunnel {
                    port: 8080,
                    label: None,
                    auto_start: false,
                    archived: false,
                    favorite: false,
                    site: None,
                    tags: Vec::new(),
                },
            ],
        };
        store.save(&state).unwrap();

        let (loaded, warning) = store.load();

        assert!(warning.is_none());
        assert_eq!(loaded.total_created, 7);
        assert_eq!(loaded.tunnels, state.tunnels);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 覆盖保存不会因文件已存在而失败() {
        let dir = temp_dir("overwrite");
        let store = StateStore::in_dir(&dir);

        store.save(&PersistedState::default()).unwrap();
        let state = PersistedState {
            total_created: 3,
            ..Default::default()
        };
        // Windows rename 语义的回归测试点。
        store.save(&state).unwrap();

        assert_eq!(store.load().0.total_created, 3);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 损坏的文件不阻断启动() {
        let dir = temp_dir("corrupt");
        std::fs::write(dir.join(STATE_FILE), "{ this is not json").unwrap();
        let store = StateStore::in_dir(&dir);

        let (state, warning) = store.load();

        assert!(state.tunnels.is_empty());
        assert!(warning.is_some(), "损坏文件必须给出可见告警");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 不认识的版本退回默认值() {
        let dir = temp_dir("version");
        std::fs::write(
            dir.join(STATE_FILE),
            r#"{"version":999,"totalCreated":5,"tunnels":[]}"#,
        )
        .unwrap();
        let store = StateStore::in_dir(&dir);

        let (state, warning) = store.load();

        assert_eq!(state.total_created, 0, "不应沿用未知版本的字段");
        assert!(warning.is_some());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 落盘内容不含公网链接字段() {
        let dir = temp_dir("no-url");
        let store = StateStore::in_dir(&dir);

        store
            .save(&PersistedState {
                version: STATE_VERSION,
                total_created: 1,
                web: Default::default(),
            tunnels: vec![PersistedTunnel {
                    port: 3000,
                    label: Some("dev".into()),
                    auto_start: true,
                    archived: false,
                    favorite: false,
                    // 站点信息也落盘，同样不得夹带公网链接。
                    site: Some(SiteInfo {
                        title: Some("dev server".into()),
                        icon: None,
                    }),
                    tags: vec!["前端".into()],
                }],
            })
            .unwrap();

        // 不变量 6 的回归测试：链接不得出现在落盘文件中。
        let raw = std::fs::read_to_string(dir.join(STATE_FILE)).unwrap();
        assert!(!raw.contains("trycloudflare"));
        assert!(!raw.contains("publicUrl"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 无路径时降级为内存模式() {
        let store = StateStore::new(None);

        let (state, warning) = store.load();

        assert!(state.tunnels.is_empty());
        assert!(warning.is_some(), "降级应告知用户配置不会保存");
        // 写入静默成功，不报错。
        store.save(&PersistedState::default()).unwrap();
    }
}
