# 内嵌 HTTP 服务与特别端口

> 状态：`[planned]`

## 目标

在 Rust 侧起一个进程内 HTTP 服务，监听本机端口，作为 Web 控制台的后端；
并把这个端口作为「特别端口」纳入现有的隧道体系，但排除在普通映射列表之外。

## 技术选型

新增依赖：

| crate | 用途 | 说明 |
|---|---|---|
| `axum` | HTTP 服务 | 基于已有的 tokio；`serve` + `with_graceful_shutdown` |
| `tower-http` | 静态文件 / 安全响应头 | 只启用 `fs`、`set-header` feature |

`tokio` 需补 `net` feature（`TcpListener`）。

不引入 `axum-session` 之类的会话库：本场景只有一个用户、一种权限，
自己用 `HashMap<SessionId, Instant>` 管理即可，少一个依赖少一份攻击面。

## 特别端口的建模

### 不复用 `Tunnel` 列表，单独存

理由：`Tunnel` 的所有消费方（映射页、历史页、计数、归档、标签）都假设条目是
「用户的服务」。把控制台混进去，每个消费方都要加一层「除非它是特别的」判断，
这种散落的例外分支正是 bug 的温床。

```rust
/// Web 控制台的配置与运行态，独立于普通映射。
pub struct WebConsole {
    /// 内嵌服务监听的本机端口
    port: u16,
    /// 备注
    label: Option<String>,
    /// token 的哈希（不存明文，见 02-security）
    token_hash: Option<String>,
    /// 服务与隧道的运行态
    status: WebStatus,
    /// 公网链接，仅运行时为 Some，**不落盘**
    public_url: Option<String>,
    /// 开机是否自动开启（默认 false）
    auto_start: bool,
}
```

落盘部分（`state.json` 新增 `web` 段，`serde(default)`，不递增 version）：
`port` / `label` / `token_hash` / `auto_start`。
`public_url` 与 `status` 是运行态，不落盘（同不变量 6）。

### 端口选择

默认建议一个不常用端口（如 `17650`），但允许用户改。
开启前检查：

1. 端口是否已被普通映射占用 → 拒绝，提示换一个
2. 端口是否被本机其他程序占用 → `TcpListener::bind` 失败时给出可操作的错误文案

## 生命周期

```
开启：
  校验 token 已设置  →  bind 本机端口  →  起 axum 服务
  →  复用 cloudflared::spawn(port) 拿公网 URL  →  状态转 Running

关闭：
  发出 shutdown 信号（服务停止接受新连接并退出）
  →  kill cloudflared 子进程（走 registry 现有路径）
  →  释放端口，状态转 Stopped
```

**三条必须能关闭的路径**（不变量 5 的延伸）：

| 场景 | 处理 |
|---|---|
| 用户手动关闭 | 走正常关闭流程 |
| 应用退出 | `shutdown_all` 里一并触发，不能只管普通映射 |
| 隧道进程崩溃 | 隧道死了但 HTTP 服务还活着——此时应把状态标为失败并**主动停掉服务**，<br>不留一个「监听着但外部访问不到」的半死状态 |

关闭用 `tokio::sync::watch` 通道 + `axum::serve(...).with_graceful_shutdown(...)`。
持有 `watch::Sender` 即可从任意位置触发关闭。

## 命令

| command | 参数 | 返回 | 职责 |
|---|---|---|---|
| `web_console_status` | - | `WebConsoleView` | 当前配置与运行态（token 只回传「是否已设置」） |
| `set_web_console_port` | `port: u16` | - | 改监听端口，仅在未运行时可改 |
| `set_web_console_label` | `label?: string` | - | 改备注 |
| `regenerate_web_token` | - | `string` | 生成新 token，**明文只在此刻返回一次** |
| `start_web_console` | - | `WebConsoleView` | 开启服务并建立隧道 |
| `stop_web_console` | - | - | 关闭 |
| `set_web_auto_start` | `enabled: bool` | - | 开机自动开启 |

`WebConsoleView` 刻意不含 `token_hash`，只含 `hasToken: bool`——
前端没有任何理由拿到哈希。

## 与 registry 的关系

隧道部分复用 `cloudflared::spawn`，但**不进 `TunnelRegistry`**：
registry 的每个方法都是为「用户的映射」写的（计数、归档、标签、分区）。
控制台的隧道由 `WebConsole` 自己持有 `Child` 与监视任务，
监视逻辑（`expected_exit` 标记、按 pid kill）与 registry 一致，可抽公共函数复用。

`tunnel_counts` 的 `active` **不计入**控制台——那个数字的语义是
「我对外暴露了几个自己的服务」，混进控制台会让它变得难以解释。

## 验收标准

- [ ] 开启后 `http://127.0.0.1:<port>` 可访问，公网 URL 可访问
- [x] 该端口不出现在映射页、历史页、`list_tunnels` 的结果里（控制台在注册表之外，结构上不可能出现）
- [x] `tunnel_counts.active` 不因控制台开启而增加（同上）
- [x] 关闭后端口立即释放 —— `web_console.rs::关闭后端口立即释放`
- [ ] 应用退出后无残留监听、无残留 cloudflared 进程
- [x] 隧道进程被强杀后，服务自动停止且状态为失败 —— `watch_tunnel` 代码路径；**未做真实强杀实测**
- [x] 端口被占用时给出可操作的错误文案，不是 panic —— `serve()` 返回「换一个端口再试」；映射占用在 `set_web_console_port` 阶段就拦下

## 回滚

`web/` 模块整体删除，`state.json` 的 `web` 段因 `serde(default)` 可安全忽略。

> **验收进度（2026-09-10）**：未打勾的两条需要真实公网环境实测，
> 属于用户侧验收，不由自动化测试覆盖。
