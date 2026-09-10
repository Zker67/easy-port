# M5：打包与开源发布

> 状态：`[done]`（构建与仓库就绪；`commit` / `push` / 建 Release 待用户授权）

## 目标

产出可分发的 Windows 安装包，并把仓库补齐到可开源状态。

## 不变量

- 不打包 `cloudflared` 二进制（不变量 4）：保持安装体积轻量，运行时检测引导。
- 仓库任何文件不得出现私有侧标识与真实隧道链接（工作区去敏铁律 + 不变量 6）。

## 实现清单

### 1. 打包配置

- `tauri.conf.json` 的 `bundle.targets` 由 `"all"` 收敛为 Windows 实际需要的目标。
  `"all"` 在 Windows 上会尝试 nsis + msi，msi(WiX) 需额外下载工具链且对中文路径敏感；
  优先 `["nsis"]`，构建通过后再评估是否加 msi。
- 补 `bundle` 元数据：`publisher`、`copyright`、`shortDescription`、`longDescription`、
  `homepage`、`category`，这些会出现在安装包属性里。
- 确认 `productName` / `identifier` / `version` 三处与 `package.json`、`Cargo.toml` 一致。

### 2. 版本一致性

`package.json`、`src-tauri/Cargo.toml`、`tauri.conf.json` 三处版本号需一致（当前均 `0.1.0`）。
发布 `0.1.0` 时保持不变即可，记入文档避免后续漂移。

### 3. 执行构建并验证

```bash
npm run tauri build
```

验证点：
- 构建成功，产出 `src-tauri/target/release/bundle/nsis/*.exe`。
- 记录安装包体积（用于确认「不打包 cloudflared」的轻量目标）。
- 产物**不**提交进仓库（`.gitignore` 已忽略 `src-tauri/target/`）。

### 4. 开源发布准备

| 项 | 处理 |
|---|---|
| LICENSE | 已有 MIT，确认 `Cargo.toml` 的 `license` 字段一致 |
| README | 补「下载安装」章节，指向 Releases；更新路线表状态 |
| 去敏检查 | 全仓 grep 私有侧标识、真实 `trycloudflare.com` 链接、本机绝对路径 |
| docs | 把 M4 的持久化事实同步到 `docs/operations.md`（配置表）与 `docs/api.md`（新 command） |

### 5. 发布动作边界

**本阶段只做到「产出可分发产物 + 仓库就绪」。**

`git commit` / `git push` / 创建 GitHub Release 属于需用户显式授权的动作，
计划执行时不自动进行，完成后向用户报告并等待指示。

## 验收标准

1. `npm run tauri build` 成功，安装包存在且体积合理。
2. 三处版本号一致。
3. 去敏 grep 无命中。
4. `README.md` 路线表 M4/M5 状态更新，新增下载章节。
5. `docs/operations.md`、`docs/api.md` 与代码实际行为一致。

## 回滚说明

M5 改动均为配置与文档，无业务逻辑。回滚还原 `tauri.conf.json` 与文档即可。
构建产物在 `target/` 下，删除目录即可清理。

---

## 执行记录

### 验收结果

| 标准 | 结果 |
|---|---|
| 1 构建成功且体积合理 | ✅ `EasyPort_0.1.0_x64-setup.exe` **2.0 MB**（主程序 5.9 MB） |
| 2 三处版本号一致 | ✅ 均为 `0.1.0` |
| 3 去敏 grep 无命中 | ✅ 无私有侧标识、无本机绝对路径、无真实隧道链接 |
| 4 README 路线表与下载章节 | ✅ M4/M5 标记完成，新增「下载安装」 |
| 5 docs 与代码一致 | ✅ `api.md` 补全 10 个 command 与落盘 schema；`operations.md` 补配置、测试与排查 |

`bundle.targets` 由 `"all"` 收敛为 `["nsis"]`，一次构建通过，未触发 WiX 工具链下载。

### 未执行（按计划边界，需用户授权）

- `git commit` / `git push`
- 创建 GitHub Release 并上传安装包

### 遗留

- 仅验证 Windows x64。macOS / Linux 目标未构建，`bundle.targets` 亦未包含。
- 安装包未做代码签名，Windows SmartScreen 首次运行会告警。
