# Web 端界面

> 状态：`[planned]`

## 定位

手机浏览器里用的**轻量控制面**，不是桌面端的移植。
场景是「人在外面，想看看哪些映射还开着 / 把某个服务打开」，
不是「在手机上完成完整配置」。

因此：功能少、页面少、加载快。

## 构建产物的组织

### 方案：独立入口，编进二进制

```
src-web/                  # Web 端源码，与桌面端 src/ 分开
├── main.tsx
├── App.tsx               # 登录 + 列表两个视图，不引路由库
└── components/
vite.config.ts            # 增加第二个 build 目标，产出 dist-web/
```

构建产物用 `include_dir!` 编进 Rust 二进制，按固定映射表提供，
**不做文件系统路径拼接**（02-security：从根上杜绝路径穿越）。

### 为什么不复用桌面端的 `src/`

复用会把整套桌面依赖拖进手机端：`@tauri-apps/api`（在浏览器里根本不可用）、
zustand、TanStack Query、shadcn 全家桶。手机端只有两个视图，
为它拉进 500 KB 的 JS 不合理。

**共用的只有类型**：把 `Tunnel` 等接口抽到 `src/lib/tunnel-types.ts`，
两端都从这里导入，保证字段不漂移。逻辑不共用。

## 页面

### 登录

单个 token 输入框 + 提交。

- `type="password"`，带显示/隐藏切换（手机上输 43 位随机串很容易输错）
- 支持粘贴（大多数人会从密码管理器复制过来）
- 失败只显示一句固定文案，不区分原因（02-security）
- 成功后种 cookie 并跳到列表

### 列表

每条映射显示：端口、备注、站点标题、状态、公网链接（可复制 / 点开）。
操作按方案 B 只有一个：**开 / 关**。

- 断开是破坏性操作 → 二次确认，与桌面端一致
- 无标签编辑、无归档、无新建——这些桌面端做
- 顶部一个「退出登录」

### 样式

不引 Tailwind 与组件库，手写一份最小 CSS（预计 100 行内）。
理由：两个视图撑不起一套设计系统，且手机端首要指标是加载速度。
配色沿用桌面端的 CSS 变量值（蓝色主色），保持观感一致。

必须做的移动端适配：
- `viewport` meta，禁止缩放导致的布局错乱
- 点击目标不小于 44×44 CSS px
- 深色模式跟随 `prefers-color-scheme`

## 接口

Web 端与内嵌服务之间是普通 HTTP JSON，**不是** Tauri IPC：

| 方法 | 路径 | 说明 |
|---|---|---|
| `POST` | `/api/login` | body `{token}`，成功种 cookie |
| `POST` | `/api/logout` | 清除 session |
| `GET` | `/api/tunnels` | 列表（复用 `Tunnel` 的 serde 输出） |
| `POST` | `/api/tunnels/:id/start` | 开启已有映射 |
| `POST` | `/api/tunnels/:id/stop` | 断开 |

除 `/api/login` 外全部走鉴权中间件（`middleware::from_fn_with_state`）。

**`/api/tunnels` 的响应要剔除不该外传的字段**——目前 `Tunnel` 的字段都可以给，
但将来若新增敏感字段，这里必须同步收敛。加一个显式的 `WebTunnelView` 转换，
而不是直接把 `Tunnel` 序列化出去，免得将来加字段时忘了这一层。

## 验收标准

- [ ] 手机浏览器实际访问，登录 → 看列表 → 开关一条映射走通
- [x] 未登录直接访问 `/api/tunnels` → 401 —— `web_console.rs::未登录访问受保护接口一律_401`
- [x] 首屏 JS 体积 < 50 KB（gzip）—— 实测 11.62 KB（React alias 到 preact/compat 后）
- [ ] 深色 / 浅色两种系统设置下观感正常
- [x] 静态资源全部来自 `include_dir!`，无文件系统读取 —— `assets.rs` 单元测试断言路径穿越取不到文件
- [x] 桌面端与 Web 端的 `Tunnel` 类型来自同一份定义 —— `src/lib/tunnel-types.ts`

## 回滚

`src-web/` 与 vite 的第二个 build 目标一并删除，Rust 侧去掉静态资源路由。

> **验收进度（2026-09-10）**：未打勾的两条是视觉与真机项，
> 按项目约定交由用户在真实浏览器 / 手机上确认。
