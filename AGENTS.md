# AGENTS.md

本文件是 AI 助手的项目入口文档。每次新会话应先阅读本文件，再阅读 `README.md` 和相关计划文档。

## 项目定位

Easy Port 是一个 Tauri 2 桌面应用，把本机任意端口映射到一条公网 HTTPS 链接供其他机器访问，支持随时断开并显示活跃映射数量。

穿透能力**不自研**，通过管理外部 `cloudflared` 子进程实现（Quick Tunnel 模式，免服务器免注册）。项目的核心复杂度在于**子进程生命周期管理**：spawn、stdout 解析取链接、健康检查、kill、应用退出时的兜底清理。

## 项目不变量

以下决策已定稿，偏离前必须先与用户确认：

1. **穿透引擎抽象**：`cloudflared` 是当前唯一实现，但必须置于 provider 抽象层之后，为后续接入 frp / ngrok 预留位置，不允许把 cloudflared 特有逻辑散落到 UI 层。
2. **不引入独立后端进程**：进程管理属于 Rust 侧职责，不额外拉起 Node 进程。
   如需 HTTP 服务（如 Web 远程控制台），只能是 Rust 进程内的内嵌服务（axum），
   且必须能随应用退出而关闭（不变量 5 的延伸）。
   *2026-09-10 由「不引入 Express」改写：原文语境只覆盖「不拉 Node 进程」，
   不覆盖进程内内嵌服务这种情况。*
3. **不引入数据库**：持久化仅隧道配置与计数，使用本地 JSON 文件；不引入 SQLite / Drizzle。
4. **cloudflared 内嵌进主程序**（2026-09-10 经用户明确要求，两次取代先前决策：先由「不打包」改为 sidecar，再改为内嵌）：
   通过 `include_bytes!`（cargo feature `embed-cloudflared`，默认开启）编进 `easy-port.exe`，
   主程序因此由 6 MB 增至约 60 MB，首次运行时释放到 app data 的 `engine/` 目录再 spawn。
   目的是**单文件即可运行**——免安装版拷走一个 exe 就能用，不依赖同级目录的任何文件。
   **回退路径不可删**：开发期（`tauri dev`）与关掉 feature 的构建都必须能用 PATH 上的 cloudflared。
   二进制不入库（见 `.gitignore`），构建前用 `node scripts/fetch-cloudflared.mjs` 获取。
5. **进程不可泄漏**：任何创建子进程的路径都必须有对应的清理路径，包括应用崩溃与强制退出场景。
6. **链接即敏感信息**：Quick Tunnel 链接是公开可访问的，日志、错误信息、截图和文档中不得留存真实隧道 URL。
7. **不使用原生控件**：步长切换器（`<input type="number">` 的原生箭头）、下拉菜单（`<select>`）、悬停提示（`title` 属性）与**滚动条**一律用自建样式或组件——`ui/number-field.tsx`、`ui/select.tsx` 与 `ui/tooltip.tsx`。原生控件由浏览器绘制，不受主题 token 控制、深浅色下观感不一致、命中区域过小，与桌面应用的一致性要求冲突。
   **`title` 属性不得再出现在 `src/` 的任何 JSX 里**，统一用 `<Hint label="…">` 包裹。
   `Hint` 不替代 `aria-label`：tooltip 只在悬停/聚焦时出现，朗读器与触屏用户依赖的仍是 `aria-label`，两者都要写。
   包 `disabled` 的按钮时需外套一层 `<span>`——disabled 元素收不到指针事件，否则「为什么点不了」这条最该解释的提示反而不显示。
   滚动条在 `index.css` 的 `@layer base` 里用 `::-webkit-scrollbar-*` 覆写（WebView2 是 Chromium 内核）；
   `src-web/` 因为跑在用户自己的浏览器里，内核不确定，标准属性与 WebKit 伪元素都要写。
8. **自建标题栏**：窗口设 `decorations: false`，标题栏由 `components/titlebar.tsx` 绘制。改动它时必须同时保证：拖拽区域（`data-tauri-drag-region`）足够大、三个窗口控制按钮对应的 `core:window:allow-*` 权限齐全、最大化状态跟随 `onResized` 更新。缺权限时按钮会静默失效。
9. **站点探测只走本机回环**：`tunnel/site.rs` 抓标题与图标时只请求 `http://127.0.0.1:<port>`，**不得改成请求公网隧道 URL**。走公网会把流量绕经 Cloudflare、暴露链接到额外的日志面，且目标不是网页时毫无意义。探测失败一律返回 `None`，不阻断建立隧道。
10. **站点信息落盘但每次启动校验**（2026-09-10 经用户要求，取代原「site 不落盘」决策）：`site` 存入 `state.json` 使列表启动即有名字可认；启动后 `spawn_site_refresh` 重新探测，**不一致才覆盖**，**探测失败保留旧值**。不要因为 `publicUrl` 不落盘就把 `site` 一并当成运行态清掉——旧链接是死链（有害），旧标题只是过时（仍可辨认）。
11. **Web 控制台暴露的是应用自己的控制面**，风险高于普通映射：拿到 URL 与 token 的人能在本机开关任意端口的公网映射。因此 —— token 必须是 CSPRNG 生成且只存 Argon2id 哈希、校验走常数时间、限速必须有不依赖请求头的全局兜底（`CF-Connecting-IP` 可伪造）、失败响应不区分原因、Web 端只能开关**已有**映射且**不能改控制台自身配置**。完整清单见 `plans/2026-09-10-web-console/02-security.md`，**删改其中任何一条前先读那份文档**。

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
