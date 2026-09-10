# AGENTS.md

本文件是 AI 助手的项目入口文档。每次新会话应先阅读本文件，再阅读 `README.md` 和相关计划文档。

## 项目定位

Easy Port 是一个 Tauri 2 桌面应用，把本机任意端口映射到一条公网 HTTPS 链接供其他机器访问，支持随时断开并显示活跃映射数量。

穿透能力**不自研**，通过管理外部 `cloudflared` 子进程实现（Quick Tunnel 模式，免服务器免注册）。项目的核心复杂度在于**子进程生命周期管理**：spawn、stdout 解析取链接、健康检查、kill、应用退出时的兜底清理。

## 项目不变量

以下决策已定稿，偏离前必须先与用户确认：

1. **穿透引擎抽象**：`cloudflared` 是当前唯一实现，但必须置于 provider 抽象层之后，为后续接入 frp / ngrok 预留位置，不允许把 cloudflared 特有逻辑散落到 UI 层。
2. **不引入 Express**：进程管理属于 Rust 侧职责，不额外拉起 Node 进程。
3. **不引入数据库**：持久化仅隧道配置与计数，使用本地 JSON 文件；不引入 SQLite / Drizzle。
4. **不打包 cloudflared 二进制**：运行时检测 + 界面引导安装，保持安装体积轻量。
5. **进程不可泄漏**：任何创建子进程的路径都必须有对应的清理路径，包括应用崩溃与强制退出场景。
6. **链接即敏感信息**：Quick Tunnel 链接是公开可访问的，日志、错误信息、截图和文档中不得留存真实隧道 URL。

## 技术栈

React 19 + TypeScript + Vite + Tailwind CSS 4 + shadcn/ui + Zustand + TanStack Query，Rust 侧 Tauri 2。完整选型表见 [README.md](./README.md#技术选型)。

## 启动流程

1. 阅读 `README.md`，确认项目目标、使用方法和文档结构。
2. 阅读 `docs/README.md` 和 `docs/architecture/source-of-truth.md`，确认信息归属。
3. 阅读 `plans/README.md`，确认是否存在进行中的计划。
4. 如任务涉及外部资料，阅读 `references/README.md` 和对应来源索引。
5. 改代码前先理解现有结构、风格、测试入口和错误处理方式。

## 目录职责

| 目录/文件 | 用途 | 默认权限 |
|---|---|---|
| `.agent/rules/` | 项目级 AI 规则 | 只读，除非任务要求维护规则 |
| `.agent/README.md` | `.agent/` 使用说明 | 只读 |
| `docs/` | 当前代码和程序的长期文档 | 读写 |
| `references/` | 外部文档和外部仓库参考 | 读写 |
| `plans/` | 计划、设计和改进追踪 | 读写 |
| `README.md` | 项目说明 | 读写 |

## 规范约定

- 默认使用简体中文沟通和编写文档；代码标识符、命令和外部专名保持原文。
- 保持 diff 聚焦，每处改动都对应任务目标、缺陷修复或必要验证。
- 非平凡改动先写计划到 `plans/`，并同步维护 `plans/README.md`。
- 改代码后按风险运行定向测试、类型检查、lint 或构建。
- 不提交真实凭据、机器路径、运行态数据、缓存或个人笔记。
- 默认不新增依赖；确需新增时说明原因、影响面和验证方式。

## 文档维护

- `README.md` 面向人类使用者，描述目标、使用方式和主要结构。
- `AGENTS.md` 面向 AI 助手，描述协作入口、约束和项目不变量。
- `docs/` 面向当前项目事实，承载结构、运行、接口和长期专题文档。
- `docs/architecture/source-of-truth.md` 规定同类信息的主维护位置，其他文档只做入口或摘要。
- `references/` 面向外部资料，记录来源结构、版本、commit、许可和借鉴点；采纳后的当前项目事实应回写到 `docs/` 或源码。
- `.agent/rules/` 存放稳定、长期、可自动加载的项目规则。
- `plans/README.md` 是计划索引，新增、移动、归档计划时同步更新；完成后的稳定事实同步到 `docs/`。
