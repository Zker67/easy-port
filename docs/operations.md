# 运行与部署说明

> 本文件维护项目如何启动、配置、构建、测试和部署。README 只保留最短入口。

## 前置依赖

| 依赖 | 用途 | 是否随应用打包 |
|---|---|---|
| Node.js + npm | 前端构建 | 否（开发期） |
| Rust toolchain | Tauri 构建 | 否（开发期） |
| `cloudflared` | 穿透引擎，运行时由应用 spawn | **否**，运行时检测并引导用户安装 |

`cloudflared` 安装方式：

```bash
# Windows
winget install --id Cloudflare.cloudflared

# macOS
brew install cloudflared
```

应用启动时通过在 PATH 中查找 `cloudflared` 判断可用性；缺失时界面进入引导态，不静默失败。

## 本地启动

```bash
npm install
npm run tauri dev
```

## 配置

| 配置项 | 位置 | 说明 |
|---|---|---|
| 隧道配置与计数 | Tauri app data 目录下的 JSON 文件 | 由应用自动创建与维护，不入库 |

本项目不使用环境变量存放密钥。Quick Tunnel 模式无需 Cloudflare 账号或 token。

> 若未来接入 frp 等需要凭据的 provider，凭据只存放于 app data 目录并由操作系统权限保护，禁止写入仓库内任何文件。

## 测试与构建

```bash
npm run build          # tsc 类型检查 + vite 构建
npm run lint           # oxlint
cargo check            # 在 src-tauri/ 下执行
npm run tauri build    # 构建桌面产物
```

## 部署

本项目为本地桌面应用，通过 GitHub Releases 分发构建产物，无服务端部署环节。

## 故障排查

| 现象 | 排查方向 |
|---|---|
| 界面提示 cloudflared 不可用 | 确认 `cloudflared --version` 在终端可执行，确认 PATH 对 GUI 进程生效（Windows 下需重启应用或注销重登） |
| 隧道创建后拿不到链接 | cloudflared 的链接输出在 stderr 而非 stdout，确认两路输出都被捕获 |
| 应用退出后隧道仍存活 | 子进程清理路径失效，检查退出钩子与崩溃兜底；手动排查残留 `cloudflared` 进程 |
| 链接可打开但页面报错 | 确认本机目标端口确有服务在监听，且监听地址不是仅 `127.0.0.1` 之外的受限接口 |
