# 项目结构总览

> 当前项目的目录结构、职责边界和层级索引以本文件为准。

## 总分层

```text
项目根目录/
├── 入口与协作层
├── 项目文档层
├── 外部参考层
├── 计划层
├── 应用源码层
└── 运行与产物层
```

## 入口与协作层

| 路径 | 职责 |
|---|---|
| `README.md` | 项目总览、快速启动和文档分流 |
| `AGENTS.md` | AI 助手协作入口、约束和项目不变量 |
| `.agent/rules/` | 项目级可自动加载规则 |

## 项目文档层

| 路径 | 职责 |
|---|---|
| `docs/README.md` | 当前项目文档总入口 |
| `docs/architecture/` | 架构、结构、边界和长期技术事实 |
| `docs/operations.md` | 运行、配置、部署和运维说明 |
| `docs/api.md` | API、接口、schema 和事件合同说明 |

## 外部参考层

| 路径 | 职责 |
|---|---|
| `references/README.md` | 外部资料总索引 |
| `references/external-docs/` | 外部文档、标准、教程和资料结构 |
| `references/external-repos/` | 外部仓库、参考实现和对比分析 |

外部参考只记录来源和可借鉴点。采纳后的当前项目事实应回写到 `docs/` 或源码，不应停留在 `references/`。

## 计划层

| 路径 | 职责 |
|---|---|
| `plans/` | 计划、设计草案、执行过程和验收状态 |

## 应用源码层

M1–M8 实际落地结构：

```text
src/                            # React 前端
├── App.tsx                     # 应用外壳：标题栏 + 侧栏 + 内容区 + 底栏安全提示
├── main.tsx                    # 入口，挂载 QueryClient 与 Toaster
├── index.css                   # Tailwind 4 主题（蓝色主色 + 隧道状态色）
├── components/
│   ├── titlebar.tsx            # 自建标题栏（decorations:false，含拖拽与窗口控制）
│   ├── sidebar.tsx             # 侧栏导航：映射 / 历史 / Web / 设置，可收起为图标态、可拖拽
│   ├── pages/
│   │   ├── mappings-page.tsx   # 工作台：收藏 / 已开启 / 已关闭分区 + 筛选搜索 + 创建表单
│   │   ├── history-page.tsx    # 档案：全部记录（含运行中），可恢复 / 删除
│   │   ├── web-page.tsx        # Web 远程控制台：风险提示 + token + 端口配置 + 开关
│   │   └── settings-page.tsx   # 开机自启 + cloudflared 状态（随包 or 系统）
│   ├── engine-guard.tsx        # cloudflared 缺失时的安装引导
│   ├── create-tunnel-form.tsx  # 端口输入与创建
│   ├── port-badge.tsx          # 端口号专用渲染（纯数字，无 localhost: 与 : 前缀）
│   ├── label-editor.tsx        # 就地编辑备注（每端口一条自由文本）
│   ├── tag-editor.tsx          # 标签增删（人工分类，跨端口复用，用于筛选）
│   ├── tunnel-card.tsx         # 单条展示：四行信息（端口/URL/备注/标签）+ 操作区
│   └── ui/                     # shadcn 组件源码 + 自建 number-field / tooltip（替代原生控件与 title）
├── hooks/
│   ├── use-tunnels.ts          # TanStack Query 封装 + 启动时自动重连
│   └── use-autostart.ts        # 系统开机自启（官方 autostart 插件）
└── lib/
    ├── tunnel-types.ts         # 桌面端与 Web 端共享的类型定义
    ├── tunnel-api.ts           # IPC 类型与调用，对齐 Rust 侧
    ├── ui-store.ts             # 纯客户端 UI 偏好（侧栏收起态），zustand + localStorage
    └── utils.ts                # cn()

src-web/                        # Web 远程控制台前端，独立打包为 dist-web/
├── index.html                  # 独立入口，不复用桌面端的 index.html
├── main.tsx                    # 挂载，无 router
├── App.tsx                     # 登录页 + 映射列表两个态
├── api.ts                      # fetch 封装，走 cookie 会话
└── styles.css                  # 手写约 100 行：oklch + 深浅色 + 44px 触控目标

src-tauri/
├── build.rs                    # 校验 binaries/ 下的 cloudflared 并把路径喂给 include_bytes!
├── src/
│   ├── main.rs                 # 二进制入口
│   ├── lib.rs                  # Builder 接线 + 引擎释放 + 状态注入 + 退出兜底清理
│   ├── commands.rs             # 19 个隧道 command（Web 侧另有 8 个），见 docs/api.md
│   ├── store.rs                # state.json 原子读写（M4）
│   ├── tunnel/
│   │   ├── provider.rs         # 引擎无关类型与错误（不变量 1）
│   │   ├── cloudflared.rs      # 内嵌释放 / spawn / 抓 stderr 取链接 / kill
│   │   ├── site.rs             # 本机站点探测：标题与 favicon（不变量 9、10）
│   │   └── registry.rs         # 注册表、计数、进程监视与落盘
│   └── web/                    # Web 远程控制台（不变量 11）
│       ├── mod.rs              # 隧道监视与统一关闭路径
│       ├── auth.rs             # token 生成 / Argon2id 哈希 / 会话 / 两级限速
│       ├── console.rs          # 控制台状态与持久化（只存哈希）
│       ├── server.rs           # axum 路由、鉴权中间件、暴露面收敛
│       ├── assets.rs           # include_dir 静态资源（结构上杜绝路径穿越）
│       └── commands.rs         # 8 个 Tauri command，仅桌面端可调用
└── tests/
    ├── tunnel_e2e.rs           # 真实公网连通性集成测试（需外网）
    ├── registry_lifecycle.rs   # 崩溃感知与持久化往返（不需外网）
    └── web_console.rs          # Web 控制台鉴权回归（不需外网）

scripts/
├── fetch-cloudflared.mjs       # 取穿透引擎二进制到 src-tauri/binaries/（构建前必跑）
└── fetch-shadcn.mjs            # 从 registry 取 shadcn 组件源码
```

### 关键边界

- **provider 抽象**：`tunnel/provider.rs` 定义引擎无关的 trait，UI 层与 `commands.rs` 只依赖该抽象，不得直接引用 `cloudflared.rs` 的具体类型。
- **计数唯一来源**：活跃映射数量由 `tunnel/registry.rs` 持有，前端只读展示，不在 UI 层维护第二份计数。
- **进程清理**：所有 spawn 必经 `registry`，保证退出时可遍历清理（不变量 5）。
- **持久化只存意图**：`store.rs` 只落盘端口、备注、自动重连开关与累计计数；
  `publicUrl` / `status` / `id` 一律不落盘。Quick Tunnel 链接随进程退出即失效，
  缓存下来只是死链，同时不变量 6 要求链接不留存。**「自动重连」重建隧道并拿新链接，不是恢复旧链接。**
- **映射页与历史页的分工**：映射页是工作台，展示所有 `archived == false` 的条目——
  **包括已断开的**，因为用户断开后往往要立刻重连，让它消失会导致「刚断开就找不到」。
  只有主动「归档」才移出映射页。历史页是档案，展示**全部**记录（含运行中）。
  归档 ≠ 删除：归档只改可见性，记录仍在历史页；`purge_archived` 才是真删除。
  历史页刻意**不提供**「下次启动时自动映射」开关——那是映射面板的属性，
  放在历史里会让人以为归档的记录也会被自动拉起。
  从历史「恢复」走的是 `create_tunnel` 复用同 id 条目的路径，`insert` 会自动取消归档。
- **进程监视**：`registry` 为每条隧道起一个 `wait()` 任务，进程非预期退出时把状态改为
  `Failed`，避免 UI 上留下「运行中」的死链。用户主动 stop / remove 会先置 `expected_exit`
  标记以区分崩溃；因监视任务持锁 `wait`，kill 路径改用登记时存下的 pid 发系统信号。
- **映射页的分区规则**：收藏 → 已开启 → 已关闭，且**收藏优先于运行状态**——
  收藏的条目即使已断开也留在收藏区。这是刻意的：否则一条收藏的映射会在开关时
  在两个分区之间跳来跳去，用户找不到它。同理「归档已关闭」跳过收藏项。
- **端口是主体单位**：一条映射就是「某个端口对外的一扇门」，端口号因此排在卡片最前
  并用 `port-badge.tsx` 专门渲染，**只显示数字**（既无 `localhost:` 也无 `:`）——
  本应用映射的永远是本机端口，这些前缀对每条都相同、不携带信息，胶囊样式本身已表明是端口。
  备注同理属于端口而非某次创建，
  用 `label-editor.tsx` 就地编辑，不必重建映射（`set_label` command）。
- **卡片的四行结构**：`tunnel-card.tsx` 的信息区固定四行——
  ① 状态点 + 端口 + 站点图标标题 + 状态徽章，② 公网 URL，③ 备注，④ 标签。
  **URL 行未运行时也占位**（显示「未运行，无公网链接」），否则状态一变下方内容会上下跳。
  操作区顺序（运行中）：断开 → 定时 → 收藏 → 自动映射 → 复制 → 浏览器打开；
  左半是「对映射做什么」，右半是「拿链接做什么」，破坏性与收纳操作收在最右。
  自动映射用 `Repeat` 图标而非 `Power`／时钟类——前者会和断开撞语义，后者会和定时关闭撞。
- **备注与标签不是一回事**：备注是每端口一条的自由文本，标签是跨端口复用的人工分类。
  标签走「随用随建」——没有独立标签表，`all_tags` 从现有条目现算，
  最后一个端口移除某标签后它自然消失。筛选取并集，详见 [api.md](../api.md)。
- **两套「状态」不要混**：Rust 侧的数据走 TanStack Query + `state.json`；
  纯界面偏好（侧栏收起态）走 `lib/ui-store.ts` 的 zustand + localStorage。
  界面偏好不该进 `state.json`——它既不需要 Rust 侧知道，也不该混进隧道配置。
- **站点信息的存活期**：`site` 与 `publicUrl` 的处理刻意不同——链接不落盘（旧的是死链），
  站点标题与图标**落盘**（旧的只是过时，仍能认出是哪个服务）。
  启动时先用落盘值填上，再由 `spawn_site_refresh` 重新探测；不一致才覆盖，
  探测失败保留旧值。改这里前先读不变量 10，别把两者当成同一类数据。
- **Web 控制台在注册表之外**：`WebConsole` 不是 `TunnelRegistry` 里的一条隧道——
  它是应用自己的控制面，不该出现在映射页、不计入活跃数量、不被「归档全部」波及。
  放进注册表会让每个消费者都长出「除非是那条特殊的」分支。
  但它的配置**要跟着 `state.json` 一起落盘**，所以 `Inner` 持有一份 `web` 快照，
  否则任何一次隧道变动的 `snapshot()` 都会把控制台配置抹掉（已有回归测试守着）。
- **Web 端暴露面是显式收敛的**：`WebTunnelView` 是从 `Tunnel` 显式转换而非直接序列化，
  将来给 `Tunnel` 加敏感字段时不会自动外泄（也因此不含 favicon——上百 KB 的 data URI
  不值得让手机端过公网拉）。接口只有「列出 + 开 + 关」，没有新建、删除、改配置。
- **明文 token 只存在一瞬**：生成时返回给界面显示一次，落盘的只有 Argon2id 哈希。
  `set_token_hash` 同时 `revoke_all()`——换了 token 却让旧会话继续用，换 token 就没意义了。
- **引擎解析**：`cloudflared.rs` 用 `OnceLock<Option<PathBuf>>` 存释放出的路径，
  `lib.rs` 在 setup 阶段调 `extract_embedded` 并 `set_sidecar` 写入；未设或释放失败时
  `program()` 退回字符串 `"cloudflared"` 交给系统 PATH 解析（不变量 4 的回退路径）。
  释放按体积比对决定是否跳过，避免每次启动都写 53 MB。

新增源码目录后，应在本文件补充职责说明；如果子目录超过两个稳定模块，建议在该目录内新增 `README.md`。

## 运行与产物层

运行脚本、构建配置、部署文件和生成产物按项目实际情况补充。构建产物、缓存、日志、数据库、真实配置和本地路径默认不入库。
