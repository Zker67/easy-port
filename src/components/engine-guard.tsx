import { openUrl } from "@tauri-apps/plugin-opener";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { AlertTriangle, Copy, ExternalLink, RefreshCw } from "lucide-react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

const INSTALL_CMD = "winget install --id Cloudflare.cloudflared";
const DOCS_URL =
  "https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/downloads/";

interface Props {
  onRetry: () => void;
  isRetrying: boolean;
}

/** cloudflared 不可用时的引导页。不静默失败，给出可操作的下一步。 */
export function EngineGuard({ onRetry, isRetrying }: Props) {
  return (
    <div className="flex min-h-screen items-center justify-center p-6">
      <Card className="w-full max-w-lg">
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <AlertTriangle className="size-5 text-status-pending" />
            未检测到 cloudflared
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <p className="text-sm text-muted-foreground">
            Easy Port 依赖 cloudflared 建立公网隧道。它没有随应用打包，需要单独安装一次。
          </p>

          <div className="space-y-2">
            <p className="text-sm font-medium">在终端执行：</p>
            <div className="flex items-center gap-2">
              <code className="selectable flex-1 rounded-md bg-muted px-3 py-2 font-mono text-xs">
                {INSTALL_CMD}
              </code>
              <Button
                variant="outline"
                size="icon-sm"
                onClick={() => {
                  void writeText(INSTALL_CMD);
                  toast.success("命令已复制");
                }}
                aria-label="复制安装命令"
              >
                <Copy />
              </Button>
            </div>
          </div>

          <p className="text-xs text-muted-foreground">
            安装完成后可能需要重启本应用，PATH 变更才会对已运行的程序生效。
          </p>

          <div className="flex gap-2">
            <Button onClick={onRetry} disabled={isRetrying}>
              <RefreshCw className={isRetrying ? "animate-spin" : ""} />
              重新检测
            </Button>
            <Button variant="outline" onClick={() => void openUrl(DOCS_URL)}>
              <ExternalLink />
              其他安装方式
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
