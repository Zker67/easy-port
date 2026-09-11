import { openUrl } from "@tauri-apps/plugin-opener";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import {
  AlertTriangle,
  CheckCircle2,
  Copy,
  ExternalLink,
  HardDrive,
  Package,
  RefreshCw,
  XCircle,
} from "lucide-react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Hint } from "@/components/ui/tooltip";
import type { EngineStatus } from "@/lib/tunnel-api";
import { cn } from "@/lib/utils";

const INSTALL_CMD = "winget install --id Cloudflare.cloudflared";
const DOCS_URL =
  "https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/downloads/";

/** 字节数转成人看的体积，用于说明「这 53 MB 是什么」 */
function formatSize(bytes: number): string {
  return `${(bytes / 1024 / 1024).toFixed(0)} MB`;
}

/**
 * 引擎来源的三种形态。
 *
 * 拆出来是因为 `bundled === false` 有两种截然不同的成因：
 * 轻量版构建本来就没内嵌（用户**应当**自己装），
 * 与一体版内嵌了却释放失败（这是**故障**，需要排查）。
 * 早先只有一个 bundled 布尔值，界面无法区分，用户看到同一句话却面对不同处境。
 */
function resolveSource(engine: EngineStatus) {
  if (engine.bundled) {
    return {
      kind: "embedded" as const,
      title: "随应用内置",
      tone: "text-status-online",
      icon: Package,
      desc: "cloudflared 已编进主程序，首次运行时释放到应用数据目录。拷走一个 exe 就能用，不依赖同级目录的任何文件。",
    };
  }
  if (engine.embedded) {
    return {
      kind: "fallback" as const,
      title: "已回退到系统版本",
      tone: "text-status-pending",
      icon: AlertTriangle,
      desc: "这个构建内置了 cloudflared，但本次运行没能释放出来（应用数据目录不可写、或被安全软件拦截），已改用系统 PATH 上的版本。功能不受影响，但「单文件自足」这次没有成立。",
    };
  }
  return {
    kind: "system" as const,
    title: "使用系统安装的版本",
    tone: "text-muted-foreground",
    icon: HardDrive,
    desc: "这是不含引擎的轻量版构建（关闭了 embed-cloudflared），依赖系统 PATH 上的 cloudflared。",
  };
}

/**
 * 引擎页：把「穿透靠什么实现、这次用的是哪一份」讲清楚。
 *
 * 从设置页拆出来独立成页（2026-09-11 用户要求）：引擎是这个应用的核心依赖，
 * 也是出问题时第一个要查的地方，塞在设置页的一个小节里既讲不开也不好找。
 */
export function EnginePage({
  engine,
  onRecheck,
  isRechecking,
}: {
  engine: EngineStatus | undefined;
  onRecheck: () => void;
  isRechecking: boolean;
}) {
  const source = engine ? resolveSource(engine) : null;
  const SourceIcon = source?.icon;

  return (
    <div className="space-y-6">
      {/* 概览：状态、版本、重新检测 */}
      <section className="space-y-3">
        <div className="flex items-center justify-between gap-4">
          <h2 className="text-sm font-semibold">穿透引擎</h2>
          <Button
            variant="outline"
            size="sm"
            onClick={onRecheck}
            disabled={isRechecking}
          >
            <RefreshCw className={isRechecking ? "animate-spin" : ""} />
            重新检测
          </Button>
        </div>

        <Card>
          <CardContent className="space-y-4 py-4">
            <div className="flex items-center gap-2">
              {engine?.available ? (
                <CheckCircle2 className="size-4 shrink-0 text-status-online" />
              ) : (
                <XCircle className="size-4 shrink-0 text-status-error" />
              )}
              <span className="font-mono text-sm font-medium">cloudflared</span>
              {engine?.available ? (
                <Badge variant="secondary">{engine.version ?? "版本未知"}</Badge>
              ) : (
                <Badge variant="destructive">不可用</Badge>
              )}
            </div>

            <p className="text-xs leading-relaxed text-muted-foreground">
              Easy Port 自己不实现内网穿透，而是管理 Cloudflare 官方的{" "}
              <span className="font-mono">cloudflared</span> 子进程。
              每建立一条映射就拉起一个进程，断开时结束它——
              应用退出时也会兜底清理，不留残留进程。
            </p>
          </CardContent>
        </Card>
      </section>

      {/* 来源：这次用的是哪一份，以及「单文件自足」是否成立 */}
      {source && SourceIcon && (
        <section className="space-y-3">
          <h2 className="text-sm font-semibold">本次运行的来源</h2>
          <Card>
            <CardContent className="space-y-4 py-4">
              <div className="flex items-start gap-2">
                <SourceIcon className={cn("mt-0.5 size-4 shrink-0", source.tone)} />
                <div className="min-w-0 space-y-1">
                  <p className={cn("text-sm font-medium", source.tone)}>
                    {source.title}
                  </p>
                  <p className="text-xs leading-relaxed text-muted-foreground">
                    {source.desc}
                  </p>
                </div>
              </div>

              <dl className="space-y-2 border-t pt-3 text-xs">
                <div className="flex gap-2">
                  <dt className="w-24 shrink-0 text-muted-foreground">
                    可执行文件
                  </dt>
                  <dd className="selectable min-w-0 flex-1 break-all font-mono">
                    {engine?.path ?? "—"}
                  </dd>
                </div>
                <div className="flex gap-2">
                  <dt className="w-24 shrink-0 text-muted-foreground">
                    构建内嵌
                  </dt>
                  <dd className="min-w-0 flex-1">
                    {engine?.embedded ? (
                      <>
                        <span className="text-status-online">是</span>
                        {engine.embeddedSize != null && (
                          <>
                            ，主程序里带了一份{" "}
                            <span className="font-mono">
                              {formatSize(engine.embeddedSize)}
                            </span>{" "}
                            的副本
                          </>
                        )}
                      </>
                    ) : (
                      <>否，轻量版构建，需系统自行安装</>
                    )}
                  </dd>
                </div>
                <div className="flex gap-2">
                  <dt className="w-24 shrink-0 text-muted-foreground">
                    解析顺序
                  </dt>
                  <dd className="min-w-0 flex-1 leading-relaxed">
                    先用释放到应用数据目录的内置副本，取不到再找系统 PATH。
                    回退路径保留是为了开发期和轻量版构建仍然可用。
                  </dd>
                </div>
              </dl>
            </CardContent>
          </Card>
        </section>
      )}

      {/* 未装引擎时的安装引导：只在真的需要时出现 */}
      {engine && !engine.available && (
        <section className="space-y-3">
          <h2 className="text-sm font-semibold">安装 cloudflared</h2>
          <Card>
            <CardContent className="space-y-3 py-4">
              <p className="text-xs leading-relaxed text-muted-foreground">
                {engine.embedded
                  ? "内置副本释放失败，且系统 PATH 上也没有 cloudflared。可以先检查应用数据目录是否可写，或手动安装一份作为退路："
                  : "未检测到 cloudflared，在终端执行以下命令安装："}
              </p>
              <div className="flex items-center gap-2">
                <code className="selectable flex-1 rounded-md bg-muted px-3 py-2 font-mono text-xs">
                  {INSTALL_CMD}
                </code>
                <Hint label="复制安装命令">
                  <Button
                    variant="outline"
                    size="icon-sm"
                    onClick={async () => {
                      try {
                        await writeText(INSTALL_CMD);
                        toast.success("命令已复制");
                      } catch {
                        toast.error("复制失败，可手动选中命令复制");
                      }
                    }}
                    aria-label="复制安装命令"
                  >
                    <Copy />
                  </Button>
                </Hint>
                <Hint label="查看官方下载页与其他安装方式">
                  <Button
                    variant="outline"
                    size="icon-sm"
                    onClick={async () => {
                      try {
                        await openUrl(DOCS_URL);
                      } catch {
                        toast.error("打开浏览器失败");
                      }
                    }}
                    aria-label="其他安装方式"
                  >
                    <ExternalLink />
                  </Button>
                </Hint>
              </div>
              <p className="text-xs text-muted-foreground">
                装好后点右上角「重新检测」，无需重启应用。
              </p>
            </CardContent>
          </Card>
        </section>
      )}

      {/* 工作模式：解释为什么链接每次都变、为什么不用登录 */}
      <section className="space-y-3">
        <h2 className="text-sm font-semibold">工作模式</h2>
        <Card>
          <CardContent className="space-y-3 py-4 text-xs leading-relaxed">
            <div className="flex gap-2">
              <span className="w-24 shrink-0 text-muted-foreground">
                Quick Tunnel
              </span>
              <span className="min-w-0 flex-1">
                免服务器、免注册，不需要 Cloudflare 账号或 token，
                因此本应用也不保存任何凭据。
              </span>
            </div>
            <div className="flex gap-2">
              <span className="w-24 shrink-0 text-muted-foreground">域名</span>
              <span className="min-w-0 flex-1">
                每次建立分配一条新的{" "}
                <span className="font-mono">*.trycloudflare.com</span>{" "}
                域名，进程退出即失效。
                <span className="text-foreground">
                  旧链接无法恢复
                </span>
                ，「自动重连」重建的是隧道而非同一条链接。
              </span>
            </div>
            <div className="flex gap-2">
              <span className="w-24 shrink-0 text-muted-foreground">可见性</span>
              <span className="min-w-0 flex-1">
                链接是公开的，任何拿到的人都能访问，没有密码保护。
                它也会出现在 Cloudflare 的边缘日志里，不要当作秘密分发。
              </span>
            </div>
          </CardContent>
        </Card>
      </section>
    </div>
  );
}
