# 穿透引擎内嵌进主程序

> 状态：`[done]`

## 目标

用户原话：「cloudflared要一体化在exe里面」。
先前一轮已把 cloudflared 改为 Tauri `externalBin` sidecar 随包分发，
但 sidecar 是**主程序同级目录的另一个文件**——免安装版拷走单个 exe 就跑不起来。
本轮要求真正的单文件。

## 不变量

- 修订**不变量 4**（本轮第二次修订）：由 sidecar 改为 `include_bytes!` 内嵌。
- **回退到 PATH 的路径不可删**：开发期与 `--no-default-features` 构建都依赖它。
- 不变量 5：释放出的引擎仍必须经 `registry` spawn，退出时可遍历清理。

## 实现清单

### 构建期

`src-tauri/build.rs`：

- cargo feature `embed-cloudflared`（默认开启）。
- 查找 `binaries/cloudflared-<target-triple><ext>`，缺失时**直接 panic 并给出提示**，
  而不是让 `include_bytes!` 在编译期抛一条难懂的路径错误。
- 通过 `cargo:rustc-env=EASY_PORT_CLOUDFLARED=<绝对路径>` 把路径交给源码。

二进制仍**不入库**（`.gitignore`），用 `node scripts/fetch-cloudflared.mjs` 获取。

### 运行期

`src-tauri/src/tunnel/cloudflared.rs`：

- `EMBEDDED: &[u8]` 由 `include_bytes!(env!("EASY_PORT_CLOUDFLARED"))` 提供。
- `extract_embedded(dir)` 写到 app data 的 `engine/`：
  **按体积比对决定是否跳过**，避免每次启动都写 53 MB；
  写入用「先写 `.tmp` 再 rename」，Windows 下 rename 前先删目标。
  Unix 下补 `0o755` 权限位。
- `SIDECAR: OnceLock<Option<PathBuf>>` 存最终路径；`lib.rs` 在 setup 阶段调用
  `extract_embedded` 后 `set_sidecar` 写入。
- `program()` 返回该路径，未设或释放失败时退回字符串 `"cloudflared"` 交给 PATH 解析。
- `is_bundled()` 供设置页显示引擎来源。

## 验收标准

- [x] `easy-port.exe` 由 6 MB 增至约 60 MB，证明二进制确实编了进去。
- [x] **单文件自足性实测**：把 `easy-port.exe` 单独拷到一个空目录（该目录内无 cloudflared），
      清空 `%APPDATA%\com.zker67.easyport\engine\`，运行后该目录出现 53 MB 的 `cloudflared.exe`。
- [x] 设置页显示引擎来源为随包版本。
- [x] `cargo build --no-default-features` 仍能构建，且走 PATH 回退。
- [x] `cargo test` 28 项通过。

## 体积说明

| 产物 | 体积 |
|---|---|
| 免安装主程序 | 约 60 MB |
| NSIS 安装包 | 约 16 MB |

安装包更小是因为 NSIS 对内嵌数据做了压缩。这个反直觉的差值值得记着，
否则下次看到「安装包比主程序小」会以为漏打包了。

## 回滚

`cargo build --no-default-features` 即可退回纯 PATH 模式，无需改代码。
彻底回退到 sidecar 需恢复 `tauri.conf.json` 的 `externalBin` 配置。
