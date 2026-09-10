# Easy Port

> **AI 助手**：请先阅读 [AGENTS.md](./AGENTS.md)，再开始任何操作。

## 项目简介

把本机任意端口一键映射到一条公网 HTTPS 链接，供其他机器访问；随时断开，并实时显示当前活跃映射数量。

## 目录

- [需求概述](#需求概述)
- [功能特性](#功能特性)
- [使用方法](#使用方法)
- [配置说明](#配置说明)
- [文档结构](#文档结构)
- [开发路线](#开发路线)

## 需求概述

### 核心需求

- [ ] **映射**：输入本机端口（如 `3000`），生成一条外网可访问的 HTTPS 链接。
- [ ] **断开**：随时终止任意一条映射，释放对应隧道。
- [ ] **计数**：界面实时显示当前活跃映射数量与历史累计数量。

### 技术选型

| 层级 | 技术 | 说明 |
|---|---|---|
| 前端 | React 19 + TypeScript + Vite | 桌面端 WebView 界面 |
| 样式 | Tailwind CSS 4 + shadcn/ui | 组件与设计系统 |
| 状态 | Zustand（客户端）+ TanStack Query（隧道状态轮询） | 客户端 UI 状态与 Rust 侧数据分离 |
| 后端 | Tauri 2（Rust） | 子进程生命周期管理、stdout 解析、IPC |
| 穿透引擎 | cloudflared Quick Tunnel | 免服务器、免注册，自动分配 `*.trycloudflare.com` 域名 |
| 数据存储 | 本地 JSON 文件（`tauri-plugin-fs` app data 目录） | 仅存隧道配置与计数，数据量小，不引入数据库 |
| 自动化 | `tsc` 类型检查 + `oxlint` + `cargo check` | 详见 [docs/operations.md](./docs/operations.md) |

> **不引入的依赖**：Express（Rust 侧直接管理进程，无需额外 Node 进程）、SQLite/Drizzle（持久化数据量极小，JSON 足够）。见 [AGENTS.md](./AGENTS.md) 的项目不变量。

## 功能特性

<!-- 随开发进度补充。当前为骨架阶段，尚未实现。 -->

## 使用方法

### 前置依赖

本工具依赖外部命令 `cloudflared`，**不随应用打包**。首次运行时应用会检测其是否可用，缺失时在界面给出安装引导。

```bash
# Windows 安装 cloudflared
winget install --id Cloudflare.cloudflared
```

### 开发

```bash
# 安装依赖
npm install

# 启动开发环境
npm run tauri dev

# 类型检查与 lint
npm run build
npm run lint

# 构建产物
npm run tauri build
```

## 配置说明

### AI 行为配置

- **协作入口**：`AGENTS.md`，AI 首次进入项目时先读取。
- **规则文件**：`.agent/rules/*.md`，使用 YAML frontmatter + Markdown。
- **目录说明**：`.agent/README.md`，说明 `.agent/` 的组织方式与规则写法。

### 项目文档

`docs/` 承载当前代码和程序的长期文档。`README.md` 只做项目总览和入口分流，`AGENTS.md` 只做 AI 协作约束；同一类信息的主维护位置以 `docs/architecture/source-of-truth.md` 为准。

### 外部参考

`references/` 用于记录可参考的外部文档和外部仓库。重要来源可以按自身结构建立子目录，记录上游目录、版本、commit、许可、借鉴点和采纳结论；采纳后的当前项目事实应回写到 `docs/` 或源码。

## 文档结构

```text
项目根目录/
├── AGENTS.md                 # AI 协作入口
├── README.md                 # 项目说明
├── docs/                     # 当前代码和程序的长期文档
│   ├── README.md
│   ├── api.md
│   ├── operations.md
│   └── architecture/
│       ├── README.md
│       ├── source-of-truth.md
│       └── project-structure.md
├── references/               # 外部文档和外部仓库参考
│   ├── README.md
│   ├── external-docs/
│   │   └── README.md
│   └── external-repos/
│       └── README.md
├── plans/                    # 计划、设计和改进追踪
│   └── README.md
└── .agent/                   # AI 项目规则
    ├── README.md
    └── rules/
```

## 开发路线

| 阶段 | 目标 | 状态 |
|---|---|---|
| M0 | 文档骨架与技术选型定稿 | ✅ |
| M1 | Tauri + React 工程初始化，`cloudflared` 可用性检测 | 待开始 |
| M2 | 单条隧道：创建 / 解析链接 / 断开 | 待开始 |
| M3 | 多隧道并发管理 + 活跃数量与累计计数 | 待开始 |
| M4 | 配置持久化、开机恢复、错误态处理 | 待开始 |
| M5 | 打包与开源发布 | 待开始 |

详见 [plans/README.md](./plans/README.md)。
