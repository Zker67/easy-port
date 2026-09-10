# API 与接口说明

> 本文件维护项目 API、schema、事件合同和跨模块接口。
> 这里的「API」指前端与 Rust 侧之间的 Tauri command 合同，本项目无 HTTP 服务端。

## Tauri command 列表

前端调用封装在 `src/lib/tunnel-api.ts`，类型与本表一一对应。

| command | 参数 | 返回 | 职责 |
|---|---|---|---|
| `check_engine` | - | `EngineStatus` | 检测 cloudflared 是否可用及版本（随包内嵌优先，回退 PATH） |
| `startup_warning` | - | `string \| null` | 启动时读取配置产生的告警，供提示一次 |
| `url_timeout_secs` | - | `u64` | 建立隧道的最长等待秒数，供前端等待提示引用 |
| `create_tunnel` | `port: u16`, `label?: string`, `expireMinutes?: u32` | `Tunnel` | 建立映射；同端口已有历史记录时复用该条目。给定 `expireMinutes` 时同时设定定时关闭 |
| `set_expiry` | `id: string`, `minutes?: u32` | `string \| null` | 设定/取消定时关闭，返回到期时刻；`null`/`0` 表示取消。仅对活跃映射有效 |
| `stop_tunnel` | `id: string` | - | 断开并回收子进程，条目转为 `stopped` |
| `remove_tunnel` | `id: string` | - | 移除记录（同时回收进程） |
| `set_auto_start` | `id: string`, `enabled: bool` | - | 设置该条是否在下次启动时自动重建 |
| `restore_tunnels` | - | `RestoreOutcome[]` | 重建所有标记自动重连的条目 |
| `list_tunnels` | - | `Tunnel[]` | 全部条目，按创建时间倒序 |
| `auto_start_flags` | - | `Record<string, bool>` | 各条自动重连开关状态，键为隧道 id |
| `set_label` | `id: string`, `label?: string` | - | 修改备注；`null` 或空串即清除。与运行状态无关，随时可改 |
| `set_tags` | `id: string`, `tags: string[]` | - | 覆盖式设置标签；裁空白、去重（大小写不敏感）、超 5 个截断 |
| `all_tags` | - | `string[]` | 当前用到的全部标签，按名称排序。从条目现算，无独立标签表 |
| `set_favorite` | `id: string`, `favorite: bool` | - | 收藏/取消收藏；收藏的条目在映射页单独成区，且不参与「归档已关闭」 |
| `set_archived` | `id: string`, `archived: bool` | - | 归档/取消归档单条；活跃映射不允许归档 |
| `archive_inactive` | - | `usize` | 归档所有已断开/失败的条目，返回归档条数 |
| `purge_archived` | - | `usize` | 彻底删除所有已归档记录，返回删除条数 |
| `tunnel_counts` | - | `TunnelCounts` | 活跃数与历史累计数 |

命令统一以 `Result<T, String>` 返回，错误值是已本地化的用户可读文本（源自 `TunnelError`）。

**错误文案约定**：每条错误都应回答「我现在该做什么」。只陈述现象而不给出下一步动作的文案视为缺陷。
`UrlTimeout(u64)` 携带秒数而非硬编码在文案里，避免与 `cloudflared::URL_TIMEOUT_SECS` 漂移（有单元测试守护）。

## 数据结构

```ts
type TunnelStatus =
  | { kind: "starting" }
  | { kind: "running" }
  | { kind: "stopped" }
  | { kind: "failed"; message: string };

interface Tunnel {
  id: string;              // 每次运行重新生成，不持久化
  port: number;
  label: string | null;    // 备注，随时可通过 set_label 修改；空串会收敛为 null
  publicUrl: string | null; // 仅 running 时为非空
  status: TunnelStatus;
  createdAt: string;        // RFC 3339；恢复出的条目为 restored-NNNN
  expiresAt: string | null; // 定时关闭到期时刻；纯运行态，不落盘
  archived: boolean;        // 已归档：从「映射」页隐藏，仍在「历史」页
  favorite: boolean;        // 收藏：在「映射」页独立成区
  site: SiteInfo | null;    // 本机服务的站点信息；探测不到为 null
  tags: string[];           // 人工分类标签，可多个、跨端口复用
}

interface SiteInfo {
  title: string | null;  // 网页 <title>
  icon: string | null;   // favicon 的 data URI
}

interface TunnelCounts {
  active: number;       // starting + running
  totalCreated: number; // 历史累计，跨重启保留
}

interface RestoreOutcome {
  port: number;
  ok: boolean;
  error: string | null;
}

interface EngineStatus {
  available: boolean;
  version: string | null;
  engine: string;      // 恒为 "cloudflared"
  path: string | null; // 可执行文件位置，供设置页展示
  bundled: boolean;    // true = 用的是内嵌释放出的副本；false = 回退到系统 PATH
}
```

### 备注（`label`）与标签（`tags`）是两种东西

不要合并，也不要把其中一个当成另一个的简写：

| | 备注 `label` | 标签 `tags` |
|---|---|---|
| 基数 | 每个端口**一条** | 每个端口**多个**（上限 5） |
| 复用 | 不跨端口 | **跨端口复用**，同名即同一分类 |
| 用途 | 自由文本，说明这个端口是干什么的 | 人工分类，用于筛选 |
| 命令 | `set_label` | `set_tags`（覆盖式） |

标签走**「随用随建」**：没有独立的标签表，`all_tags` 从现有条目现算。
因此某标签的最后一个端口移除它后，该标签自然从筛选栏消失，不需要额外的清理逻辑。

去重是**大小写不敏感**的（`Dev` 与 `dev` 视为同一个，保留首次出现的写法）——
否则筛选栏会出现一眼看去重复的标签。

前端筛选取**并集**：选中多个标签时命中任一即显示。
交集在标签数量少时几乎选不出东西，不符合「点几个标签看看这几类」的直觉。

### 站点探测

建立隧道成功后 Rust 侧 `tokio::spawn` 一次探测，**只请求 `http://127.0.0.1:<port>`**
（见不变量 9），解析 `<title>` 与 favicon 并把图标转成 data URI 写回条目。
超时 2 秒、HTML 最多读 64 KB、图标最多 128 KB；任一环节失败都返回 `None`，
不影响隧道本身——被映射的目标本来就不一定是网页。

探测是异步的，因此 `create_tunnel` 返回时 `site` 可能还是上次的值（或 `null`），
前端靠 `list_tunnels` 的轮询拿到后续结果。

#### 落盘与校验规则

`site` 会落盘，但**每次启动都重新校验**，三条规则缺一不可：

| 时机 | 行为 |
|---|---|
| 启动加载 | 直接沿用落盘值，列表立刻有名字和图标可认，不必等探测 |
| 启动后校验 | `restore_tunnels` 顺带触发 `spawn_site_refresh`，对每条**存有站点信息**的条目重新探测 |
| 拿到新结果 | 与旧值 `!=` 才覆盖并落盘；相同则不写盘，避免每次启动都改写 `state.json` |

**探测失败时保留旧值**，不清空。目标服务可能只是还没起来或响应慢，
此时把标题清成空白会让列表退化成一排端口号——显示一个可能过时的名字是更好的失败模式。

这与 `publicUrl` 的处理刻意不同：链接失效后是**死链**（点了会报错，有害），
而旧标题最多是**过时**（还能认出是哪个服务，无害且有用）。
不要因为两者都是「运行态派生数据」就套用同一套规则。

开机自启不走本项目的 command，由官方 `@tauri-apps/plugin-autostart` 提供
（`enable` / `disable` / `isEnabled`），封装在 `src/hooks/use-autostart.ts`。

Rust 侧使用 `serde(rename_all = "camelCase")`，`TunnelStatus` 用
`tag = "kind", content = "message"` 的内部标签表示，前端类型需与之保持一致。

## 持久化 schema

落盘文件：app data 目录下的 `state.json`，由 `src-tauri/src/store.rs` 维护。

```jsonc
{
  "version": 1,          // 格式版本，不匹配时退回默认值
  "totalCreated": 12,    // 历史累计创建数
  "tunnels": [
    {
      "port": 3000,
      "label": "dev",
      "autoStart": true,
      "archived": false,
      "favorite": false,
      "site": { "title": "我的开发服务器", "icon": "data:image/png;base64,…" },
      "tags": ["前端", "常用"]
    }
  ]
}
```

`archived`、`favorite`、`site` 与 `tags` 都用 `serde(default)` 声明，因此**新增这些字段都没有递增 version**：
旧 state.json 缺字段时按 `false` / `null` 处理，可直接读取。后续加字段沿用这个做法。

**刻意不持久化的字段**：`id`（每次运行重新生成）、`publicUrl`、`status`、`expiresAt`。

`site` 落盘的目的是**启动即有名字可认**，不是长期缓存——见下方「站点探测」的校验规则。

`expiresAt` 不落盘的原因与链接同理：隧道随进程退出即失效，跨重启保留一个到期时间没有意义。
定时任务由 Rust 侧 `tokio::spawn` 持有 `AbortHandle`；stop / remove / 崩溃 / 复用条目
四条路径都必须 `abort()` 它，否则旧定时器会在到点时误杀重建后的同 id 隧道。

`publicUrl` 不落盘是硬性约束，原因有二：

1. Quick Tunnel 域名由边缘在进程存活期间临时分配，进程退出即失效，存下来只是死链。
2. 不变量 6 要求链接不留存于任何文件。

因此**「自动重连」重建的是隧道，不是旧链接**——每次都会分配新链接。

## 变更规则

- 增删 command 时同步更新本表与 `src/lib/tunnel-api.ts`。
- 改动落盘 schema 必须递增 `version`，并在 `store.rs` 处理旧版本（当前策略：不认识则退回默认值）。
- API 或 schema 变更应写清兼容性影响；计划阶段草案写入 `plans/`，定稿后同步到本文件。
