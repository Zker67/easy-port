import { Trash2 } from "lucide-react";

import { TunnelCard } from "@/components/tunnel-card";
import { Button } from "@/components/ui/button";
import { Hint } from "@/components/ui/tooltip";
import { useCreateTunnel, usePurgeArchived } from "@/hooks/use-tunnels";
import { isActive, type Tunnel } from "@/lib/tunnel-api";

/**
 * 历史页：**全部**记录的总账，包括正在运行的。
 *
 * 与映射页的分工：映射页是「工作台」（只看未归档的），
 * 历史页是「档案」（什么都在）。这里不提供断开、定时、自动重建等操作，
 * 只提供回顾、恢复到映射页、以及彻底删除。
 */
export function HistoryPage({
  tunnels,
  totalCreated,
}: {
  tunnels: Tunnel[];
  totalCreated: number;
}) {
  const create = useCreateTunnel();
  const purge = usePurgeArchived();

  // 运行中的排在前面，其余按创建时间（list 已倒序）
  const items = [...tunnels].sort(
    (a, b) => Number(isActive(b.status)) - Number(isActive(a.status)),
  );
  const archivedCount = items.filter((t) => t.archived).length;

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-sm font-semibold">历史记录</h2>
          <p className="text-xs text-muted-foreground">
            全部 {items.length} 条记录（含运行中）；历史累计创建 {totalCreated}{" "}
            条
          </p>
        </div>
        {/* disabled 的按钮收不到指针事件，tooltip 不会触发——
            而「为什么点不了」恰恰是此时最该解释的。故用 span 承载触发。 */}
        <Hint
          label={
            archivedCount === 0
              ? "没有已归档的记录可清空。先在映射页归档，记录才会进入这里"
              : "彻底删除所有已归档的记录；运行中与未归档的不受影响"
          }
        >
          <span className="inline-flex">
            <Button
              variant="outline"
              size="sm"
              onClick={() => purge.mutate()}
              disabled={archivedCount === 0 || purge.isPending}
            >
              <Trash2 />
              清空已归档{archivedCount > 0 ? `（${archivedCount}）` : ""}
            </Button>
          </span>
        </Hint>
      </div>

      {items.length === 0 ? (
        <div className="space-y-2 rounded-lg border border-dashed px-6 py-16 text-center">
          <p className="text-sm text-muted-foreground">还没有任何记录。</p>
          <p className="text-xs text-muted-foreground">
            建立过的映射都会留在这里，随时可以恢复。
          </p>
        </div>
      ) : (
        <div className="space-y-2">
          {items.map((t) => (
            <TunnelCard
              key={t.id}
              tunnel={t}
              autoStart={false}
              variant="history"
              onReconnect={() =>
                create.mutate({ port: t.port, label: t.label ?? undefined })
              }
              isReconnecting={create.isPending}
            />
          ))}
        </div>
      )}

    </div>
  );
}
