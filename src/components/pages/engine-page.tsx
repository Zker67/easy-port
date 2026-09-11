import { openUrl } from "@tauri-apps/plugin-opener";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import {
  AlertTriangle,
  Copy,
  ExternalLink,
  HardDrive,
  Package,
  RefreshCw,
  XCircle,
} from "lucide-react";
import { toast } from "sonner";

import { InfoRow, RowNote } from "@/components/info-row";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Hint } from "@/components/ui/tooltip";
import type { EngineStatus } from "@/lib/tunnel-api";
import { cn } from "@/lib/utils";

const INSTALL_CMD = "winget install --id Cloudflare.cloudflared";
const DOCS_URL =
  "https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/downloads/";

function formatSize(bytes: number): string {
  return `${(bytes / 1024 / 1024).toFixed(0)} MB`;
}

/**
 * 引擎来源的三种形态。
 *
 * `bundled === false` 有两种截然不同的成因，必须分开讲：
 * 轻量版本来就没内嵌（用户**该**自己装），与一体版内嵌了却释放失败（这是**故障**）。
 */
function resolveSource(engine: EngineStatus) {
  if (engine.bundled) {
    return {
      title: "随应用内置",
      note: "拷走一个 exe 就能用",
      icon: Package,
      tone: "text-status-online",
      ring: "ring-status-online/30",
      bg: "bg-status-online/10",
    };
  }
  if (engine.embedded) {
    return {
      title: "已回退到系统版本",
      note: "内置副本释放失败，检查应用数据目录是否可写",
      icon: AlertTriangle,
      tone: "text-status-pending",
      ring: "ring-status-pending/30",
      bg: "bg-status-pending/10",
    };
  }
  return {
    title: "使用系统安装的版本",
    note: "轻量版构建，不含内置引擎",
    icon: HardDrive,
    tone: "text-muted-foreground",
    ring: "ring-border",
    bg: "bg-muted",
  };
}

/**
 * 引擎页。
 *
 * 排版原则：状态一眼可见，细节可扫读，不写大段说明文字。
 * 初版堆了三段解释性 prose，全是同一号同一色的小字，
 * 用户反馈「文字太乱、不清晰」——改为状态区 + 键值行。
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
  const Icon = source?.icon ?? XCircle;
  const unavailable = engine && !engine.available;

  return (
    <div className="space-y-4">
      {/* 状态区：进页面第一眼要回答「现在能不能用、用的哪一份」 */}
      <Card>
        <CardContent className="py-5">
          <div className="flex items-start gap-4">
            <div
              className={cn(
                "flex size-11 shrink-0 items-center justify-center rounded-xl ring-1",
                unavailable
                  ? "bg-destructive/10 ring-destructive/30"
                  : cn(source?.bg, source?.ring),
              )}
            >
              <Icon
                className={cn(
                  "size-5",
                  unavailable ? "text-destructive" : source?.tone,
                )}
              />
            </div>

            <div className="min-w-0 flex-1 space-y-0.5">
              <div className="flex items-center gap-2">
                <span className="font-mono text-sm font-medium">
                  cloudflared
                </span>
                {engine?.available && engine.version && (
                  <span className="font-mono text-xs text-muted-foreground">
                    {engine.version}
                  </span>
                )}
              </div>
              <p
                className={cn(
                  "text-sm font-medium",
                  unavailable ? "text-destructive" : source?.tone,
                )}
              >
                {unavailable ? "不可用" : source?.title}
              </p>
              {!unavailable && source && (
                <p className="text-xs text-muted-foreground">{source.note}</p>
              )}
            </div>

            <Hint label="重新检测引擎状态">
              <Button
                variant="outline"
                size="sm"
                onClick={onRecheck}
                disabled={isRechecking}
              >
                <RefreshCw className={isRechecking ? "animate-spin" : ""} />
                检测
              </Button>
            </Hint>
          </div>
        </CardContent>
      </Card>

      {/* 安装引导：只在真的不可用时出现，且排在细节之前——此时它才是主线 */}
      {unavailable && (
        <Card>
          <CardContent className="space-y-3 py-4">
            <p className="text-xs text-muted-foreground">
              {engine.embedded
                ? "内置副本释放失败，系统里也没有。可手动装一份作为退路："
                : "在终端执行以下命令安装："}
            </p>
            <div className="flex items-center gap-2">
              <code className="selectable flex-1 rounded-md bg-muted px-3 py-2 font-mono text-xs">
                {INSTALL_CMD}
              </code>
              <Hint label="复制命令">
                <Button
                  variant="outline"
                  size="icon-sm"
                  onClick={async () => {
                    try {
                      await writeText(INSTALL_CMD);
                      toast.success("命令已复制");
                    } catch {
                      toast.error("复制失败，可手动选中复制");
                    }
                  }}
                  aria-label="复制安装命令"
                >
                  <Copy />
                </Button>
              </Hint>
              <Hint label="官方下载页">
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
          </CardContent>
        </Card>
      )}

      {/* 细节：可扫读的键值对，不写成句子 */}
      <Card>
        <CardContent className="py-2">
          <dl className="divide-y">
            <InfoRow label="路径" mono>
              {engine?.path ?? "—"}
            </InfoRow>
            <InfoRow label="内置引擎">
              {engine?.embedded ? (
                <>
                  已编入主程序
                  {engine.embeddedSize != null && (
                    <span className="text-muted-foreground">
                      （{formatSize(engine.embeddedSize)}）
                    </span>
                  )}
                </>
              ) : (
                <span className="text-muted-foreground">无，需自行安装</span>
              )}
            </InfoRow>
            <InfoRow label="查找顺序">
              <span className="text-muted-foreground">
                内置副本 → 系统 PATH
              </span>
            </InfoRow>
            <InfoRow label="模式">
              <RowNote>
                Quick Tunnel
                <span className="text-muted-foreground">
                  免注册，无需 Cloudflare 账号
                </span>
              </RowNote>
            </InfoRow>
            <InfoRow label="域名">
              <RowNote>
                <span className="font-mono">*.trycloudflare.com</span>
                <span className="text-muted-foreground">
                  每次重连都是新链接，旧的无法恢复
                </span>
              </RowNote>
            </InfoRow>
          </dl>
        </CardContent>
      </Card>
    </div>
  );
}
