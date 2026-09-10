# 外壳重构与引擎内嵌 — 总览

> 状态：`[done]`
> 与 [2026-09-10-ux-polish](../2026-09-10-ux-polish/00-overview.md) 的区别：
> 那一轮明确「不加功能，只打磨」；本轮是用户逐条提出的**新需求**，会改动界面结构与分发方式。

## 背景

ux-polish 交付后，用户在真实使用中连续提出若干要求。它们不是打磨，
而是改变了应用外壳（导航结构）、映射的生命周期语义（归档 / 收藏）和分发形态（单文件）。
因此单独立卷，不追加进 ux-polish。

## 需求来源与落点

| 用户要求 | 落点 | 状态 |
|---|---|---|
| 主题色改蓝 | `src/index.css` 主色 token | `[done]` |
| 步长切换器与下拉菜单不用原生控件 | `ui/number-field.tsx`、`ui/select.tsx`；固化为不变量 7 | `[done]` |
| 顶栏不用原生 | `components/titlebar.tsx`，`decorations: false`；固化为不变量 8 | `[done]` |
| 侧栏：映射 / 历史 / 设置 | `components/sidebar.tsx` + `components/pages/` | `[done]` |
| 设置含开机自启与内置 CF 情况 | `pages/settings-page.tsx` | `[done]` |
| 映射可定时关闭 | `set_expiry` command + Rust 侧 `AbortHandle` | `[done]` |
| cloudflared 跟随应用打包 | 先 sidecar，后改内嵌；**两次修订不变量 4** | `[done]` |
| 关闭的映射不该消失；归档才移出；历史可恢复但不可自启 | `archived` 字段 + 两页分工 | `[done]` |
| 下拉菜单要真的向下展开 | `ui/select.tsx` 改 `position="popper"` | `[done]` |
| 链接可点击直达 / 提示移到底栏 / 收藏分区与筛选 / 显示网页名字图标 / CF 一体化进 exe | 见 [01-mapping-page](./01-mapping-page.md)、[02-embed-engine](./02-embed-engine.md) | `[done]` |

## 不变量变更记录

- **不变量 4** 在本轮被改了两次：`不打包` → `externalBin` sidecar → `include_bytes!` 内嵌。
  两次都由用户明确要求驱动。最终形态的目的是**单文件可运行**，
  因此「回退到 PATH」这条路径仍然不可删（开发期与关掉 feature 的构建都依赖它）。
- **新增不变量 9**：站点探测只走 `127.0.0.1`，不得改成请求公网隧道 URL。
- 新增不变量 7、8 见上表。

## 验证

- `cargo test`：28 项通过（18 单元 + 10 集成）。
- `npm run build`（tsc + vite）、`npm run lint`（oxlint）通过。
- 单文件自足性：把 `easy-port.exe` 单独放进空目录、清空 app data 的 `engine/` 后运行，
  确认引擎被释放出来且隧道可建立。
- 界面观感与交互由用户在真实窗口中验收。

## 回滚

引擎内嵌可用 `cargo build --no-default-features` 关掉，回到纯 PATH 模式；
界面改动集中在 `src/components/`，无数据迁移，`state.json` 新增字段均为 `serde(default)`，
旧文件可直接读取，回滚不会丢配置。
