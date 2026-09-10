import { useState } from "react";
import { Loader2, ShieldAlert } from "lucide-react";

import { EngineGuard } from "@/components/engine-guard";
import { HistoryPage } from "@/components/pages/history-page";
import { MappingsPage } from "@/components/pages/mappings-page";
import { SettingsPage } from "@/components/pages/settings-page";
import { WebPage } from "@/components/pages/web-page";
import { Sidebar, type Page } from "@/components/sidebar";
import { Titlebar } from "@/components/titlebar";
import { Badge } from "@/components/ui/badge";
import { Hint } from "@/components/ui/tooltip";
import {
  useAutoStartFlags,
  useEngineStatus,
  useRestoreOnLaunch,
  useTunnelCounts,
  useTunnels,
} from "@/hooks/use-tunnels";

export default function App() {
  const [page, setPage] = useState<Page>("mappings");

  const engine = useEngineStatus();
  const tunnels = useTunnels();
  const counts = useTunnelCounts();
  const autoStartFlags = useAutoStartFlags();

  // 引擎确认可用后才重建，否则每条都会以「未找到 cloudflared」失败。
  useRestoreOnLaunch(engine.data?.available === true);

  const ready = engine.data?.available === true;
  const items = tunnels.data ?? [];
  const flags = autoStartFlags.data ?? {};
  const activeCount = counts.data?.active ?? 0;

  function renderPage() {
    if (page === "settings") {
      // 设置页在引擎缺失时也必须可达——修复入口就在这里
      return (
        <SettingsPage
          engine={engine.data}
          onRecheck={() => void engine.refetch()}
          isRechecking={engine.isFetching}
        />
      );
    }

    if (engine.isPending) {
      return (
        <div className="flex h-full items-center justify-center">
          <Loader2 className="size-6 animate-spin text-muted-foreground" />
        </div>
      );
    }

    if (!ready) {
      return (
        <EngineGuard
          onRetry={() => void engine.refetch()}
          isRetrying={engine.isFetching}
        />
      );
    }

    if (page === "history") {
      return (
        <HistoryPage
          tunnels={items}
          totalCreated={counts.data?.totalCreated ?? 0}
        />
      );
    }

    if (page === "web") {
      return <WebPage />;
    }

    return <MappingsPage tunnels={items} autoStartFlags={flags} />;
  }

  return (
    // 窗口已无系统边框，这里自己补一圈描边，否则浅色主题下窗口与桌面糊在一起
    <div className="flex h-screen flex-col overflow-hidden border border-border bg-background">
      <Titlebar>
        {ready && (
          // 有映射在跑时加重显示：关掉应用会让这些链接立即失效
          <Hint
            side="bottom"
            label={
              activeCount > 0
                ? `${activeCount} 条映射正在运行，关闭应用会让这些链接立即失效`
                : "当前没有运行中的映射"
            }
          >
            <Badge variant={activeCount > 0 ? "default" : "secondary"}>
              活跃 {activeCount}
            </Badge>
          </Hint>
        )}
      </Titlebar>

      <div className="flex min-h-0 flex-1">
        <Sidebar page={page} onChange={setPage} activeCount={activeCount} />
        <main className="flex-1 overflow-y-auto">
          {/* 布局随窗口宽度伸缩：窄窗口撑满，宽窗口不至于把卡片拉得过长 */}
          <div className="mx-auto w-full max-w-5xl p-6">{renderPage()}</div>
        </main>
      </div>

      {/* 底栏：常驻安全提示，不再占用内容区 */}
      <footer className="flex shrink-0 items-center gap-2 border-t border-border bg-card/40 px-4 py-1.5 text-xs text-muted-foreground">
        <ShieldAlert className="size-3.5 shrink-0 text-status-pending" />
        <span className="truncate">
          公网链接无需密码，任何人拿到即可访问你的本机服务。请勿映射数据库、管理后台等敏感服务。
        </span>
      </footer>
    </div>
  );
}
