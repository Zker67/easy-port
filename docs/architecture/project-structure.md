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

M1 实际落地结构：

```text
src/                            # React 前端
├── App.tsx                     # 主界面：引擎守卫 / 表单 / 列表 / 计数
├── main.tsx                    # 入口，挂载 QueryClient 与 Toaster
├── index.css                   # Tailwind 4 主题（含隧道状态色）
├── components/
│   ├── engine-guard.tsx        # cloudflared 缺失时的安装引导
│   ├── create-tunnel-form.tsx  # 端口输入与创建
│   ├── tunnel-card.tsx         # 单条隧道展示与操作
│   └── ui/                     # shadcn 组件源码
├── hooks/
│   └── use-tunnels.ts          # TanStack Query 封装
└── lib/
    ├── tunnel-api.ts           # IPC 类型与调用，对齐 Rust 侧
    └── utils.ts                # cn()

src-tauri/
├── src/
│   ├── main.rs                 # 二进制入口
│   ├── lib.rs                  # Builder 接线 + 退出兜底清理
│   ├── commands.rs             # 6 个 Tauri command
│   └── tunnel/
│       ├── provider.rs         # 引擎无关类型与错误（不变量 1）
│       ├── cloudflared.rs      # spawn / 抓 stderr 取链接 / kill
│       └── registry.rs         # 活跃隧道注册表与计数
└── tests/
    └── tunnel_e2e.rs           # 真实公网连通性集成测试

scripts/
└── fetch-shadcn.mjs            # 从 registry 取 shadcn 组件源码
```

> 持久化（`storage.rs`）属 M4，尚未实现；当前隧道列表仅存于内存，重启即清空。

### 关键边界

- **provider 抽象**：`tunnel/provider.rs` 定义引擎无关的 trait，UI 层与 `commands.rs` 只依赖该抽象，不得直接引用 `cloudflared.rs` 的具体类型。
- **计数唯一来源**：活跃映射数量由 `tunnel/registry.rs` 持有，前端只读展示，不在 UI 层维护第二份计数。
- **进程清理**：所有 spawn 必经 `registry`，保证退出时可遍历清理（不变量 5）。

新增源码目录后，应在本文件补充职责说明；如果子目录超过两个稳定模块，建议在该目录内新增 `README.md`。

## 运行与产物层

运行脚本、构建配置、部署文件和生成产物按项目实际情况补充。构建产物、缓存、日志、数据库、真实配置和本地路径默认不入库。
