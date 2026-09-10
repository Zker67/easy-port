# M4 + M5 总览：持久化、恢复、错误态与发布

> 状态：`[done]`
> 范围来自 [README.md 开发路线](../../README.md#开发路线) 的 M4、M5 两行。

## 目标

| 阶段 | 目标 | 文档 |
|---|---|---|
| M4 | 配置持久化、开机恢复、错误态处理 | [01-persistence.md](./01-persistence.md) |
| M5 | 打包与开源发布 | [02-release.md](./02-release.md) |

## 前置事实（M1–M3 已完成）

- `tunnel/registry.rs` 是进程句柄与计数的唯一来源，全部状态在内存。
- `tunnel/cloudflared.rs` 负责 spawn 与从 **stderr** 解析链接。
- 重启应用后隧道列表清空，累计计数归零。

## 贯穿两阶段的关键约束

### Quick Tunnel 链接不可恢复（影响 M4 设计）

cloudflared Quick Tunnel 的 `*.trycloudflare.com` 域名由边缘在**进程存活期间**临时分配，进程退出即失效且不可复用。

因此「开机恢复」**不可能**是恢复旧链接，只能是：

1. 持久化用户**意图**（端口、备注、是否自动重连）。
2. 启动时对标记了自动重连的条目**重新 spawn**，得到**全新链接**。
3. 旧链接一律不持久化、不回显（同时满足不变量 6：链接是敏感信息）。

把这条写进代码注释和文档，避免后续误改成「缓存 URL 再复原」。

### 不变量遵守

| 不变量 | 本次如何遵守 |
|---|---|
| 1 引擎抽象 | 持久化层只存 provider 无关字段，不存 cloudflared 特有信息 |
| 3 不引入数据库 | 单个 JSON 文件 + 原子写 |
| 5 进程不泄漏 | 自动重连路径同样经 registry 登记；启动失败要清理 |
| 6 链接即敏感信息 | `public_url` 不落盘、不进日志 |

## 验收总标准

- `cargo test`、`cargo check`、`npm run build`、`npm run lint` 全绿。
- 重启应用后端口与备注仍在，自动重连项拿到新链接。
- 落盘 JSON 中不含任何 `trycloudflare.com` 字样。
- `npm run tauri build` 产出可安装包。
- 界面视觉由用户在真实环境验证（项目规则：不做 headless 截图自验）。
