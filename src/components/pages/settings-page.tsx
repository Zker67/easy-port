import { openUrl } from "@tauri-apps/plugin-opener";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import {
  CheckCircle2,
  Copy,
  ExternalLink,
  RefreshCw,
  XCircle,
} from "lucide-react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { useAutostart, useSetAutostart } from "@/hooks/use-autostart";
import type { EngineStatus } from "@/lib/tunnel-api";

const INSTALL_CMD = "winget install --id Cloudflare.cloudflared";
const DOCS_URL =
  "https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/downloads/";

export function SettingsPage({
  engine,
  onRecheck,
  isRechecking,
}: {
  engine: EngineStatus | undefined;
  onRecheck: () => void;
  isRechecking: boolean;
}) {
  const autostart = useAutostart();
  const setAutostart = useSetAutostart();

  return (
    <div className="space-y-6">
      <section className="space-y-3">
        <h2 className="text-sm font-semibold">启动</h2>
        <Card>
          <CardContent className="flex items-center justify-between gap-4 py-4">
            <div className="space-y-1">
              <Label
                htmlFor="autostart"
                className="cursor-pointer text-sm font-medium"
              >
                开机自启
              </Label>
              <p className="text-xs text-muted-foreground">
                Windows 登录后自动启动 Easy Port。
                若还要自动建立映射，需在对应映射上单独开启「下次启动时自动映射」。
              </p>
            </div>
            <Switch
              id="autostart"
              checked={autostart.data ?? false}
              disabled={autostart.isPending || setAutostart.isPending}
              onCheckedChange={(v) => setAutostart.mutate(v)}
              aria-label="开机自启"
            />
          </CardContent>
        </Card>
      </section>

      <section className="space-y-3">
        <h2 className="text-sm font-semibold">穿透引擎</h2>
        <Card>
          <CardContent className="space-y-4 py-4">
            <div className="flex items-center justify-between gap-4">
              <div className="flex items-center gap-2">
                {engine?.available ? (
                  <CheckCircle2 className="size-4 text-status-online" />
                ) : (
                  <XCircle className="size-4 text-status-error" />
                )}
                <span className="text-sm font-medium">cloudflared</span>
                {engine?.available ? (
                  <Badge variant="secondary">
                    {engine.version ?? "版本未知"}
                  </Badge>
                ) : (
                  <Badge variant="destructive">未安装</Badge>
                )}
              </div>
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

            <dl className="space-y-2 text-xs">
              <div className="flex gap-2">
                <dt className="w-20 shrink-0 text-muted-foreground">打包方式</dt>
                <dd>
                  {engine?.bundled ? (
                    <>
                      <span className="text-status-online">已随应用打包</span>
                      ，开箱即用，无需单独安装。
                    </>
                  ) : (
                    <>
                      使用系统已安装的 cloudflared（本次运行未找到随包版本，
                      已回退到 PATH）。
                    </>
                  )}
                </dd>
              </div>
              <div className="flex gap-2">
                <dt className="w-20 shrink-0 text-muted-foreground">
                  可执行文件
                </dt>
                <dd className="selectable min-w-0 flex-1 break-all font-mono">
                  {engine?.path ?? "—"}
                </dd>
              </div>
              <div className="flex gap-2">
                <dt className="w-20 shrink-0 text-muted-foreground">模式</dt>
                <dd>
                  Quick Tunnel，免服务器免注册；每次建立分配一条新的{" "}
                  <span className="font-mono">*.trycloudflare.com</span>{" "}
                  域名，进程退出即失效。
                </dd>
              </div>
            </dl>

            {!engine?.available && (
              <div className="space-y-2 rounded-lg border border-dashed p-3">
                <p className="text-xs text-muted-foreground">
                  未检测到 cloudflared，在终端执行以下命令安装：
                </p>
                <div className="flex items-center gap-2">
                  <code className="selectable flex-1 rounded-md bg-muted px-3 py-2 font-mono text-xs">
                    {INSTALL_CMD}
                  </code>
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
                </div>
              </div>
            )}
          </CardContent>
        </Card>
      </section>

      <section className="space-y-3">
        <h2 className="text-sm font-semibold">关于</h2>
        <Card>
          <CardContent className="space-y-1 py-4 text-xs text-muted-foreground">
            <p>Easy Port — 把本机端口一键映射到公网 HTTPS 链接。</p>
            <p>
              配置保存在系统 app data 目录下的{" "}
              <span className="font-mono">state.json</span>；
              公网链接不会写入任何文件。
            </p>
          </CardContent>
        </Card>
      </section>
    </div>
  );
}
