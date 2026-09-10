# 09 观察记录：超出本计划范围的事项

> 状态：`[planned]`（仅记录，不在本计划执行）

走查时发现的问题中，有几条**属于功能改动或架构调整**，不符合「不增加功能、只做人性化」的边界。
记在这里避免遗忘，是否做由后续决定。

## A. `TunnelStatus::Starting` 定义了但从未被使用

`provider.rs:13` 定义了 `Starting`，`registry.rs` 的活跃判定、`tunnel-api.ts` 的类型、
`tunnel-card.tsx:24` 的状态样式**都为它做了处理**，但**没有任何代码产生这个状态**——
`commands.rs` 的 `establish` 是拿到链接后才 `insert`，插入时直接就是 `Running`。

要让它真正可用，需把建立流程改为「先 insert 为 Starting → 异步等链接 → 回写 Running/Failed」。
好处是列表能立刻出现条目、等待可视化更自然（直接解决 01-1 的根因）。
代价是动 registry 时序与 `create_tunnel` 的返回语义，属于架构改动。

**建议**：若将来要做 01-1 的彻底版，就一并做这条；否则保留现状，
但应在 `provider.rs` 加注释说明 `Starting` 当前为预留状态，避免后人误以为是 bug。

## B. 端口探测只覆盖 `127.0.0.1`

见 [02-copy-and-errors.md](./02-copy-and-errors.md) 的 2-2。
文案层面的缓解属于本计划；真正修准（探测 `::1` / 枚举监听地址）属于行为改动。

## C. 三个轮询各自独立，固定 3 秒

`use-tunnels.ts` 里 `useTunnels` / `useTunnelCounts` / `useAutoStartFlags` 各自
`refetchInterval: 3000`，即**每 3 秒 3 次 IPC**。当前数据量下无实际性能问题，
但存在两个可优化点：

1. 三者可合并为一个返回聚合结果的 command，减少 IPC 往返。
2. 无活跃隧道时可停止轮询（`refetchInterval` 支持传函数动态返回 `false`）。

属于性能与架构优化，非人性化改进，故不纳入本计划。第 2 点很轻，若顺手可做。

## D. 无日志留存

出问题时（尤其隧道意外退出）没有可回溯的记录，用户只能看到一句错误提示。
加日志涉及新增能力与「链接不进日志」的不变量 6 约束，需要单独设计，属新功能。

## E. 未做代码签名 / 仅 Windows x64

见 [../2026-09-10-m4-m5/02-release.md](../2026-09-10-m4-m5/02-release.md) 的遗留部分。
属发布事项，非本计划范围。
