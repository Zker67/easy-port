# 计划文档索引

> 单一入口：所有实施计划、设计草案、改进追踪和验收状态都在 `plans/` 目录下管理。当前项目长期事实以 `docs/` 为准。

## 何时创建计划

- 新项目启动或需要明确架构边界时。
- 重大重构、跨模块改动或高风险变更前。
- 新功能涉及多个阶段、多个文件或需要验收标准时。

## 与其他文档的边界

- `docs/`：当前代码和程序的长期事实。计划完成后，把稳定结论同步到对应 `docs/` 文档。
- `references/`：外部文档和外部仓库参考。计划可链接参考资料，但不复制外部资料结构。

## 文档结构约定

```text
plans/
├── README.md
├── YYYY-MM-DD-feature-name/
│   ├── 00-overview.md
│   ├── 01-phase-name.md
│   └── 09-improvements.md
└── archive/
```

## 每个阶段文档建议包含

1. 目标：本阶段要达成什么。
2. 不变量：哪些行为或接口不能破坏。
3. 实现清单：文件、接口、数据流和边界条件。
4. 验收标准：如何确认完成。
5. 回滚说明：失败时如何恢复。

## 状态标注

| 标注 | 含义 |
|---|---|
| `[done]` | 已完成并验证 |
| `[partial]` | 核心可用但有遗留问题 |
| `[planned]` | 已规划，未执行 |
| `[dropped]` | 已废弃或被替代 |

## 计划列表

| 计划 | 阶段 | 状态 |
|---|---|---|
| [2026-09-10-m4-m5](./2026-09-10-m4-m5/00-overview.md) | 总览 | `[done]` |
| ├ [01-persistence](./2026-09-10-m4-m5/01-persistence.md) | M4 持久化 / 恢复 / 错误态 | `[done]` |
| └ [02-release](./2026-09-10-m4-m5/02-release.md) | M5 打包与开源发布 | `[done]` |
| [2026-09-10-ux-polish](./2026-09-10-ux-polish/00-overview.md) | 总览：不加功能的人性化打磨 | `[partial]` |
| ├ [01-feedback-and-waiting](./2026-09-10-ux-polish/01-feedback-and-waiting.md) | 等待过程与操作反馈 | `[done]` |
| ├ [02-copy-and-errors](./2026-09-10-ux-polish/02-copy-and-errors.md) | 文案、错误信息与空状态 | `[done]` |
| ├ [03-window-and-shell](./2026-09-10-ux-polish/03-window-and-shell.md) | 窗口、键盘与外壳细节 | `[partial]` |
| └ [09-improvements](./2026-09-10-ux-polish/09-improvements.md) | 超出范围的观察记录 | `[planned]` |
| [2026-09-10-shell-and-bundling](./2026-09-10-shell-and-bundling/00-overview.md) | 总览：外壳重构与引擎内嵌 | `[done]` |
| ├ [01-mapping-page](./2026-09-10-shell-and-bundling/01-mapping-page.md) | 可点链接、分区筛选、站点识别 | `[done]` |
| ├ [02-embed-engine](./2026-09-10-shell-and-bundling/02-embed-engine.md) | cloudflared 内嵌进主程序 | `[done]` |
| ├ [03-site-persistence](./2026-09-10-shell-and-bundling/03-site-persistence.md) | 站点信息落盘 + 启动校验 | `[done]` |
| └ [04-labels-and-tags](./2026-09-10-shell-and-bundling/04-labels-and-tags.md) | 端口成为持久单位：备注 + 标签筛选 | `[done]` |

M1–M3（工程初始化、单条隧道、多隧道并发与计数）在建立本目录前完成，
其稳定事实直接记录于 [docs/architecture/project-structure.md](../docs/architecture/project-structure.md)，无对应计划文档。
