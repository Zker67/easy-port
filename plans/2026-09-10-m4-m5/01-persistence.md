# M4：配置持久化、开机恢复、错误态处理

> 状态：`[done]`

## 目标

1. **持久化**：端口、备注、自动重连开关、累计计数跨重启保留。
2. **开机恢复**：启动时对标记自动重连的条目重新建立隧道（新链接）。
3. **错误态处理**：进程意外退出能被感知并反映到 UI；失败原因可读。

## 不变量

- 落盘文件中不得出现任何公网链接（不变量 6）。
- 持久化字段保持 provider 无关（不变量 1）。
- 自动重连产生的进程同样登记进 registry（不变量 5）。
- 不引入 SQLite（不变量 3）：单 JSON 文件。

## 实现清单

### 1. 新增 `src-tauri/src/store.rs`

落盘位置：`app_data_dir()/state.json`。

```rust
struct PersistedTunnel { port: u16, label: Option<String>, auto_start: bool }
struct PersistedState { version: u32, total_created: u64, tunnels: Vec<PersistedTunnel> }
```

- `version` 字段预留迁移位；当前为 `1`。
- **原子写**：先写 `state.json.tmp` 再 `rename`，避免掉电写坏。
- 读取失败（文件损坏 / 字段不兼容）**不阻断启动**，退回默认值并记一条可见告警。
- 显式不含 `public_url` / `id` / `status`：链接不落盘，id 每次运行重新生成。

### 2. registry 支持持久化与状态回写

- `TunnelRegistry` 增加 `auto_start` 字段与 `set_auto_start(id, bool)`。
- `total_created` 从落盘值恢复，语义由「本次启动累计」改为「历史累计」，同步改 UI 文案与注释。
- 变更点（insert / stop / remove / set_auto_start）后触发保存。
  保存需在锁外执行或先取快照，避免持锁做 IO。

### 3. 进程意外退出的感知（错误态处理核心）

当前只有用户主动 stop 才会改状态；cloudflared 自己崩了，UI 仍显示「运行中」，链接却已失效。

方案：spawn 成功后起一个监视任务 `child.wait()`，进程退出时把该条目状态改为
`Failed("隧道进程意外退出")` 并清空 `public_url`。

实现要点：
- 进程句柄归 registry 所有，监视任务不能同时持有 `Child`。
  采用「registry 持有 `Child`，由 registry 内部的 `wait` 任务驱动」的方式：
  `insert` 时把 `Child` 交给一个 spawn 出去的任务，任务结束后回调 registry 改状态；
  registry 保留一个 `kill` 用的句柄（`child.id()` + `AbortHandle`，或改持 `Arc<Mutex<Child>>`）。
- 用户主动 stop 造成的退出**不应**被标成 Failed，需要一个「预期退出」标记来区分。

### 4. 新增 command

| command | 用途 |
|---|---|
| `set_auto_start(id, enabled)` | 切换单条自动重连 |
| `restore_tunnels()` | 启动时由前端触发一次恢复，返回结果摘要 |

恢复放在前端触发而非 Rust 的 `setup`，理由：需要先确认引擎可用，且要能把每条的失败原因回报给 UI。

### 5. 修复：clipboard 权限缺失

`capabilities/default.json` 只声明了 `core:default` 与 `opener:default`，
但 `tunnel-card.tsx` / `engine-guard.tsx` 调用了 `plugin-clipboard-manager` 的 `writeText`。
需补 `clipboard-manager:allow-write-text`，否则复制按钮在打包产物中静默失败。

### 6. 前端

- `TunnelCard` 增加自动重连开关（shadcn `switch`，用 `scripts/fetch-shadcn.mjs` 取源码）。
- 启动时调用一次 `restore_tunnels()`，逐条 toast 成功/失败。
- 计数文案「累计」改为历史累计语义。

## 验收标准

1. 建 2 条隧道，其一开启自动重连 → 关闭应用 → 重开：两条记录都在，开启项拿到**新链接**且可访问，未开启项为「已断开」。
2. 落盘 `state.json` 中 `grep trycloudflare` 无结果。
3. 手动 `taskkill` 掉某条 cloudflared → UI 在数秒内变为失败态，而非仍显示运行中。
4. 把 `state.json` 内容改成 `{` 这类坏数据 → 应用仍能启动，退回空列表。
5. `cargo test` / `cargo check` / `npm run build` / `npm run lint` 全绿。

## 回滚说明

改动集中在新增 `store.rs` 与 `registry.rs` 的状态回写。回滚只需还原这两个文件与
`capabilities/default.json`，前端 UI 增量独立可摘除。落盘文件对旧版本无害（旧版本不读它）。

---

## 执行记录

### 与计划的偏差

1. **kill 路径改用 pid 而非 `Child::kill()`**。
   监视任务需要持有 `Child` 的锁执行 `wait()`，stop 若去抢同一把锁会永久阻塞。
   最终实现：`insert` 时先取出 `child.id()` 存进 `Process.pid`，kill 时优先 `try_lock`
   （无监视任务时走 tokio kill），拿不到锁则按 pid 发系统信号（Windows `taskkill /T /F`，
   Unix `SIGKILL`）。为此新增了 unix-only 的 `libc` 依赖。

2. **同端口复用条目**。计划未涉及。恢复出的条目是「已断开」态，用户若手动对同一端口再点映射，
   会出现两行重复记录。实现中加了 `find_by_port` + `insert(existing_id)`，原地复用并保留其
   `auto_start` 与列表位置。

3. **`created_at` 对恢复条目用 `restored-NNNN` 占位**。原始时间戳未持久化（不在计划的落盘字段内），
   但列表排序依赖该字段，故用序号占位保证顺序稳定。

### 验收结果

| 标准 | 结果 |
|---|---|
| 1 重启后配置保留、自动重连拿新链接 | 自动化覆盖（`配置与计数跨重启保留且链接不落盘`）；真机由用户验证 |
| 2 `state.json` 无 `trycloudflare` | ✅ 单元 + 集成测试各一条断言 |
| 3 进程被杀后 UI 转失败态 | ✅ `进程意外退出会被标记为失败态` |
| 4 坏数据不阻断启动 | ✅ `损坏的文件不阻断启动` |
| 5 四项检查全绿 | ✅ `cargo test` 15 项通过、`cargo check`、`npm run build`、`oxlint` 均干净 |

新增测试：`store.rs` 7 项单元测试、`registry.rs` 2 项、`tests/registry_lifecycle.rs` 4 项集成测试。

### 顺带修复

`capabilities/default.json` 缺 `clipboard-manager:allow-write-text`，
而复制按钮依赖它——打包产物中复制会静默失败。已补。
