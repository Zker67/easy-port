import { openUrl } from "@tauri-apps/plugin-opener";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { Copy, ExternalLink, Power, Trash2 } from "lucide-react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { useRemoveTunnel, useStopTunnel } from "@/hooks/use-tunnels";
import { isActive, type Tunnel, type TunnelStatus } from "@/lib/tunnel-api";
import { cn } from "@/lib/utils";

function statusMeta(status: TunnelStatus) {
  switch (status.kind) {
    case "running":
      return { text: "运行中", dot: "bg-status-online" };
    case "starting":
      return { text: "启动中", dot: "bg-status-pending animate-pulse" };
    case "stopped":
      return { text: "已断开", dot: "bg-muted-foreground" };
    case "failed":
      return { text: status.message, dot: "bg-status-error" };
  }
}

export function TunnelCard({ tunnel }: { tunnel: Tunnel }) {
  const stop = useStopTunnel();
  const remove = useRemoveTunnel();
  const meta = statusMeta(tunnel.status);
  const active = isActive(tunnel.status);

  return (
    <Card>
      <CardContent className="flex items-center gap-4 py-4">
        <div className="min-w-0 flex-1 space-y-1">
          <div className="flex items-center gap-2">
            <span className={cn("size-2 shrink-0 rounded-full", meta.dot)} />
            <span className="font-mono text-sm font-medium">
              localhost:{tunnel.port}
            </span>
            {tunnel.label && (
              <Badge variant="secondary" className="truncate">
                {tunnel.label}
              </Badge>
            )}
            <span className="truncate text-xs text-muted-foreground">
              {meta.text}
            </span>
          </div>

          {tunnel.publicUrl ? (
            <p className="selectable truncate font-mono text-xs text-muted-foreground">
              {tunnel.publicUrl}
            </p>
          ) : (
            <p className="text-xs text-muted-foreground">—</p>
          )}
        </div>

        <div className="flex shrink-0 items-center gap-1">
          {tunnel.publicUrl && (
            <>
              <Button
                variant="outline"
                size="icon-sm"
                onClick={() => {
                  void writeText(tunnel.publicUrl!);
                  toast.success("链接已复制");
                }}
                aria-label="复制链接"
              >
                <Copy />
              </Button>
              <Button
                variant="outline"
                size="icon-sm"
                onClick={() => void openUrl(tunnel.publicUrl!)}
                aria-label="在浏览器打开"
              >
                <ExternalLink />
              </Button>
            </>
          )}

          {active ? (
            <Button
              variant="destructive"
              size="icon-sm"
              onClick={() => stop.mutate(tunnel.id)}
              disabled={stop.isPending}
              aria-label="断开映射"
            >
              <Power />
            </Button>
          ) : (
            <Button
              variant="ghost"
              size="icon-sm"
              onClick={() => remove.mutate(tunnel.id)}
              disabled={remove.isPending}
              aria-label="移除记录"
            >
              <Trash2 />
            </Button>
          )}
        </div>
      </CardContent>
    </Card>
  );
}
