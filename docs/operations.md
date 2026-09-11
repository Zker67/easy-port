# 运行与部署说明

> 本文件维护项目如何启动、配置、构建、测试和部署。README 只保留最短入口。

## 前置依赖

| 依赖 | 用途 | 是否随应用打包 |
|---|---|---|
| Node.js + npm | 前端构建 | 否（开发期） |
| Rust toolchain | Tauri 构建 | 否（开发期） |
| `cloudflared` | 穿透引擎，运行时由应用 spawn | **是**，用 `include_bytes!` 内嵌进 `easy-port.exe` |
| `dist-web/` | Web 远程控制台的前端产物 | **是**，用 `include_dir!` 内嵌 |

### 构建前必须先产出 dist-web

```bash
npm run build:web    # 产出 dist-web/，约 11.6 KB gzip
```

`src-tauri/src/web/assets.rs` 里的 `include_dir!("$CARGO_MANIFEST_DIR/../dist-web")`
在**编译期**读目录，缺了它 `cargo build` 直接失败——这不是运行时才报的错。

`npm run build` 与 `npm run tauri build` 都已包含这一步；
**只有单独跑 `cargo check` / `cargo test` 时需要先手动执行一次**。
`dist-web/` 是产物，**不入库**（见 `.gitignore`）。

Web 端刻意 alias `react` → `preact/compat`：控制台要在手机上过公网加载，
桌面端那份 153 KB gzip 的包不合适，换 Preact 后降到 11.6 KB。

### 构建前必须先获取 cloudflared

二进制约 53 MB，**不入库**，构建前需先下载到 `src-tauri/binaries/`：

```bash
node scripts/fetch-cloudflared.mjs           # 从 GitHub Releases 下载最新版
node scripts/fetch-cloudflared.mjs 2026.9.0  # 指定版本
node scripts/fetch-cloudflared.mjs --local   # 从本机已装的 cloudflared 复制
```

文件名必须是 `cloudflared-<target-triple><ext>`（如
`cloudflared-x86_64-pc-windows-msvc.exe`），`build.rs` 按这个约定查找并把绝对路径
通过 `EASY_PORT_CLOUDFLARED` 环境变量交给 `include_bytes!`，脚本已处理。
缺少该文件时构建在 `build.rs` 阶段就 panic，给出明确提示而非链接期报错。

想构建不含引擎的轻量版时关掉 feature：

```bash
cargo build --release --no-default-features   # 在 src-tauri/ 下
```

**运行时解析顺序**：app data 的 `engine/cloudflared.exe`（首次运行时从内嵌数据释放，
按体积比对决定是否跳过覆写）→ 系统 PATH。
开发期（`tauri dev`）同样会释放内嵌副本；关掉 feature 构建时则只剩 PATH，因此开发机建议自备一份。

单文件自足性可这样验证：把 `easy-port.exe` 单独拷到一个空目录，清空
`%APPDATA%\com.zker67.easyport\engine\`，运行后该目录应出现 53 MB 的 `cloudflared.exe`。

`cloudflared` 安装方式：

```bash
# Windows
winget install --id Cloudflare.cloudflared

# macOS
brew install cloudflared
```

应用启动时先尝试释放内嵌副本，再判断引擎可用性；两条路径都拿不到时界面进入引导态，不静默失败。
**「引擎」页**会显示本次用的是随包版本还是系统版本，并区分「轻量版本来就没内嵌」
与「内嵌了但释放失败」——后者是故障，需要查 app data 是否可写。

## 本地启动

```bash
npm install
npm run tauri dev
```

## 配置

| 配置项 | 位置 | 说明 |
|---|---|---|
| 隧道配置与计数 | app data 目录下的 `state.json` | 由应用自动创建与维护，不入库；schema 见 [api.md](./api.md#持久化-schema) |
| 释放出的穿透引擎 | app data 目录下的 `engine/cloudflared.exe` | 首次运行时从内嵌数据写出，可安全删除（下次启动会重新释放） |
| Web 控制台配置 | `state.json` 的 `web` 段 | 端口、备注、自动开启开关与 **token 的 Argon2id 哈希**；明文 token 不落盘 |

`state.json` 只存端口、备注、自动重连开关与历史累计计数，采用「先写 `.tmp` 再 rename」的原子写。
**公网链接不落盘**：Quick Tunnel 域名随进程退出即失效，缓存无意义且违反不变量 6。

文件损坏、版本不认识或目录不可用时，应用退回空列表启动并在界面提示一次，不阻断使用。

Windows 下该目录通常为 `%APPDATA%\com.zker67.easyport\`。

本项目不使用环境变量存放密钥。Quick Tunnel 模式无需 Cloudflare 账号或 token。

> 若未来接入 frp 等需要凭据的 provider，凭据只存放于 app data 目录并由操作系统权限保护，禁止写入仓库内任何文件。

## 测试与构建

```bash
npm run build          # tsc 类型检查 + vite 构建
npm run lint           # oxlint
npm run tauri build    # 构建桌面产物

# 在 src-tauri/ 下执行
cargo check
cargo test --lib                                    # 单元测试
cargo test --test registry_lifecycle -- --nocapture # 进程生命周期与持久化
cargo test --test tunnel_e2e -- --nocapture         # 真实公网连通性测试
cargo test --test web_console -- --nocapture        # Web 控制台鉴权回归（不需外网）
cargo build --examples && cargo test --test job_object  # 强杀后子进程不残留（Windows）
```

`job_object` 需要先 `cargo build --examples`：它靠 `examples/job_orphan_probe.rs`
起一个真实进程再从外部强杀，单跑 `--test job_object` 不会自动构建 example，
缺了会跳过而非失败（跳过时 stderr 有提示）。

`registry_lifecycle` 用一个长睡的 `node` 子进程代替 cloudflared，验证崩溃感知、
主动停止不误报、配置跨重启保留与链接不落盘，**不需要外网**。

`web_console` 真起一个监听随机端口的 HTTP 服务，用真实请求验证：未登录 401、
cookie 四属性齐全、限速与认证失败响应不可区分、换 token 踢旧会话、安全响应头、
关闭后端口释放。**这些是安全措施的回归证据，删改前先读
[plans/2026-09-10-web-console/02-security.md](../plans/2026-09-10-web-console/02-security.md)。**

`tunnel_e2e` 会对真实 cloudflared 建立一条隧道并从公网回访本机，需要外网连通与本机 `node`；条件不满足时自动跳过而非失败。

构建产物（默认开启 `embed-cloudflared`）：

| 产物 | 路径 | 体积 |
|---|---|---|
| 安装包 | `src-tauri/target/release/bundle/nsis/EasyPort_<版本>_x64-setup.exe` | 约 16 MB |
| 免安装主程序 | `src-tauri/target/release/easy-port.exe` | 约 60 MB |

安装包比主程序小是因为 NSIS 对内嵌的 cloudflared 做了压缩。免安装版可单文件拷走运行。

## 获取 shadcn 组件

```bash
node scripts/fetch-shadcn.mjs <组件名>...
```

shadcn CLI 的 `add` 在 npm 11 下会执行畸形命令而必然失败，故改用只读的 `view` 取官方源码。

**取下来的源码必须人工检查后才能用**，registry 不保证与本项目环境兼容。已踩过两次：

| 组件 | 问题 | 后果 |
|---|---|---|
| `switch` | 用 `data-checked:` 变体，但 Radix 渲染的是 `data-state="checked"`；且具名 group 变体未生成 CSS | 开关完全不可用，只显示一个圆点 |
| `select` | 从 `@/app/(create)/components/icon-placeholder` 导入图标占位组件 | 路径不存在，构建直接失败 |
| `select` | `position` 默认 `item-aligned` + 硬性 `min-w-36` | 面板盖在触发器上且比它更宽，不像下拉菜单 |
| `tooltip` | 用 `data-open:` / `data-closed:` 变体，但 Radix 渲染的是 `data-state="open"` | 与 `switch` 同一个坑：动画完全不生效 |
| `tooltip` | 箭头（旋转 45° 的方块）要与气泡描边拼合，各 `side` 下都对齐很难 | 出现位置不对的小三角，已改为不渲染箭头 |

检查清单：`data-checked` / `data-unchecked` / `data-open` / `data-closed` 一律改为 `data-[state=…]`；非本项目的 import 路径需换成 `lucide-react`；Next.js 的 `"use client"` 指令可删除。
**注意 `tsc` 与 `oxlint` 发现不了第一类问题**——它编译通过但渲染不出来，必须在真实界面里确认。

一个不依赖界面的快速自查：构建后在编译产物里 grep 对应选择器，没生成规则就是踩了这个坑。

```bash
npm run build && grep -c 'data-state=open' dist/assets/index-*.css
```

> grep 时注意 CSS 里的类名是**转义**的（`.group-hover\:bg-primary\/60`），
> 直接搜 `after:absolute` 这种原样写法会假阴性。搜属性值或用 Python 正则更可靠。

同一类坑不限于 registry 组件——自己写的 Tailwind 也会中招：

| 写法 | 问题 | 后果 |
|---|---|---|
| `after:absolute` 等 `after:` 变体 | 未配 `content-['']`，不生成 `::after` 规则 | 伪元素完全不渲染，界面上什么都看不到 |

侧栏拖拽把手的指示线最初就写成了 `after:`，编译产物里 `::after` 规则数为 0。
**改用真实子元素**，不依赖伪元素。

## Web 远程控制台

在「Web」页给应用自己开一个端口并映射出去，就能在手机等其他设备上开关本机的映射。

- **必须先设置访问 token 才能开启**，没有 token 时 `start_web_console` 直接拒绝。
- token 由 CSPRNG 生成（32 字节 → 43 字符 base64url），**明文只在生成时显示一次**，
  之后只剩 Argon2id 哈希；忘了就重新生成。
- 重新生成 token 会**立即作废所有已登录会话**，旧设备需重新输入。
- 该端口不出现在「映射」页，也不计入顶栏活跃数量——它是应用自己的控制面，不是用户的服务。
- Web 端只能**开关已有映射**，不能新建、删除、改配置，更不能改控制台自身。

| 现象 | 排查方向 |
|---|---|
| 开启时报「无法监听本机 X 端口」 | 端口被占用，换一个（默认 17650） |
| 手机上打不开 | 隧道是否还在（cloudflared 挂了会连带停掉服务）；链接是否是最新的——每次开启都是新域名 |
| 反复输错 token 后正确 token 也进不去 | 触发了限速锁定（每来源 5 次失败锁 15 分钟），等待或重启应用 |
| 换了 token 后手机被登出 | 预期行为，重新输入新 token |

## 部署

本项目为本地桌面应用，通过 GitHub Releases 分发构建产物，无服务端部署环节。

## 故障排查

| 现象 | 排查方向 |
|---|---|
| 界面提示 cloudflared 不可用 | 先看引擎页显示的来源。随包版本失败时检查 app data 的 `engine/` 是否可写（被安全软件拦截、磁盘满）；回退到系统版本时确认 `cloudflared --version` 在终端可执行、PATH 对 GUI 进程生效（Windows 下需重启应用或注销重登） |
| 首次启动比平时慢几秒 | 正在把 53 MB 的内嵌引擎释放到 `engine/`，只发生一次；之后按体积比对跳过 |
| 映射卡片不显示网页名字和图标 | 目标端口不是网页、2 秒内没响应、或页面没有 `<title>`。探测失败不影响隧道本身 |
| 隧道创建后拿不到链接 | cloudflared 的链接输出在 stderr 而非 stdout，确认两路输出都被捕获 |
| 应用退出后隧道仍存活 | 正常退出看 `RunEvent::Exit` 钩子；**强制结束**看 Job Object 是否生效（`tunnel/job.rs`，启动时若创建失败会在 stderr 打日志）。排查残留：`Get-Process cloudflared` |
| 链接可打开但页面报错 | 确认本机目标端口确有服务在监听，且监听地址不是仅 `127.0.0.1` 之外的受限接口 |
| 标题栏按钮点了没反应 | `capabilities/default.json` 缺 `core:window:allow-*` 权限。缺权限时命令被拦截且不报错，需补齐后重新构建 |
| 窗口拖不动 | 标题栏的 `data-tauri-drag-region` 被子元素覆盖，或该区域宽度不足 |
| 条目显示「隧道进程意外退出」 | cloudflared 自行退出（网络中断、被安全软件结束）。重新点击映射即可，会分配新链接 |
| 重启后链接变了 | 预期行为。Quick Tunnel 每次分配的域名都是新的，无法保留旧链接 |
| 重启后配置丢失 | 检查界面启动提示；`state.json` 损坏时会退回空列表并给出告警 |
