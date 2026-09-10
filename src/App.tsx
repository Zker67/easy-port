import { Cable, Loader2 } from "lucide-react";

import { CreateTunnelForm } from "@/components/create-tunnel-form";
import { EngineGuard } from "@/components/engine-guard";
import { TunnelCard } from "@/components/tunnel-card";
import { Badge } from "@/components/ui/badge";
import {
  useEngineStatus,
  useTunnelCounts,
  useTunnels,
} from "@/hooks/use-tunnels";

export default function App() {
  const engine = useEngineStatus();
  const tunnels = useTunnels();
  const counts = useTunnelCounts();

  if (engine.isPending) {
    return (
      <div className="flex min-h-screen items-center justify-center">
        <Loader2 className="size-6 animate-spin text-muted-foreground" />
      </div>
    );
  }

  if (!engine.data?.available) {
    return (
      <EngineGuard
        onRetry={() => void engine.refetch()}
        isRetrying={engine.isFetching}
      />
    );
  }

  const items = tunnels.data ?? [];

  return (
    <div className="min-h-screen bg-background">
      <div className="mx-auto max-w-3xl space-y-6 p-6">
        <header className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Cable className="size-5" />
            <h1 className="text-lg font-semibold">Easy Port</h1>
          </div>
          <div className="flex items-center gap-2 text-xs text-muted-foreground">
            <Badge variant="secondary">
              活跃 {counts.data?.active ?? 0}
            </Badge>
            <span>累计 {counts.data?.totalCreated ?? 0}</span>
          </div>
        </header>

        <CreateTunnelForm />

        {items.length === 0 ? (
          <div className="rounded-lg border border-dashed py-16 text-center">
            <p className="text-sm text-muted-foreground">
              还没有映射。输入本机端口即可生成公网链接。
            </p>
          </div>
        ) : (
          <div className="space-y-2">
            {items.map((t) => (
              <TunnelCard key={t.id} tunnel={t} />
            ))}
          </div>
        )}

        <p className="text-center text-xs text-muted-foreground">
          链接由 cloudflared Quick Tunnel 提供，任何人拿到链接都可访问，请勿映射敏感服务。
        </p>
      </div>
    </div>
  );
}
