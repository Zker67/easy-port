import { useEffect, useMemo, useState } from "react";
import { Archive, Search, Star, Tags } from "lucide-react";

import { CreateTunnelForm } from "@/components/create-tunnel-form";
import { TunnelCard } from "@/components/tunnel-card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Hint } from "@/components/ui/tooltip";
import {
  useAllTags,
  useArchiveInactive,
  useCreateTunnel,
} from "@/hooks/use-tunnels";
import { isActive, type Tunnel } from "@/lib/tunnel-api";
import { cn } from "@/lib/utils";

type Filter = "all" | "favorite" | "running" | "stopped";

const FILTERS: { id: Filter; label: string }[] = [
  { id: "all", label: "全部" },
  { id: "favorite", label: "收藏" },
  { id: "running", label: "已开启" },
  { id: "stopped", label: "已关闭" },
];

/**
 * 映射页：展示所有**未归档**的条目，按收藏 / 已开启 / 已关闭分区。
 *
 * 已断开的映射刻意留在这里而不是立刻甩进历史：用户往往要马上重连，
 * 让它消失会导致「刚断开就找不到了」。只有主动归档后才移出本页。
 */
export function MappingsPage({
  tunnels,
  autoStartFlags,
}: {
  tunnels: Tunnel[];
  autoStartFlags: Record<string, boolean>;
}) {
  const create = useCreateTunnel();
  const archiveInactive = useArchiveInactive();
  const allTags = useAllTags();
  const [filter, setFilter] = useState<Filter>("all");
  const [keyword, setKeyword] = useState("");
  // 选中的标签，空集表示不按标签过滤
  const [activeTags, setActiveTags] = useState<string[]>([]);

  const tagOptions = allTags.data ?? [];

  // 标签被删光后要把它从选中集合里摘掉，否则会出现「选了一个不存在的标签
  // 导致列表恒为空」且用户看不到那个标签按钮、无从取消
  useEffect(() => {
    if (activeTags.length === 0) return;
    const alive = activeTags.filter((t) => tagOptions.includes(t));
    if (alive.length !== activeTags.length) setActiveTags(alive);
  }, [tagOptions, activeTags]);

  const visible = useMemo(() => {
    const kw = keyword.trim().toLowerCase();
    return tunnels
      .filter((t) => !t.archived)
      .filter((t) => {
        // 标签取并集：选中多个时，命中任一即显示。
        // 交集在标签少的时候几乎选不出东西，不符合「点几个标签看看这几类」的直觉。
        if (activeTags.length === 0) return true;
        return t.tags.some((tag) => activeTags.includes(tag));
      })
      .filter((t) => {
        if (!kw) return true;
        // 端口、备注、站点标题、标签都可搜
        return (
          String(t.port).includes(kw) ||
          (t.label ?? "").toLowerCase().includes(kw) ||
          (t.site?.title ?? "").toLowerCase().includes(kw) ||
          t.tags.some((tag) => tag.toLowerCase().includes(kw))
        );
      });
  }, [tunnels, keyword, activeTags]);

  // 收藏优先于运行状态：收藏的即使已断开也留在收藏区，避免它在两处跳来跳去
  const favorites = visible.filter((t) => t.favorite);
  const running = visible.filter((t) => !t.favorite && isActive(t.status));
  const stopped = visible.filter((t) => !t.favorite && !isActive(t.status));

  const sections = [
    { id: "favorite" as const, title: "收藏", icon: Star, items: favorites },
    { id: "running" as const, title: "已开启", icon: null, items: running },
    { id: "stopped" as const, title: "已关闭", icon: null, items: stopped },
  ].filter((s) => filter === "all" || filter === s.id);

  const inactiveCount = visible.filter(
    (t) => !isActive(t.status) && !t.favorite,
  ).length;
  const hasAny = visible.length > 0;

  function renderCard(t: Tunnel) {
    return (
      <TunnelCard
        key={t.id}
        tunnel={t}
        autoStart={autoStartFlags[t.id] ?? false}
        onReconnect={() =>
          create.mutate({ port: t.port, label: t.label ?? undefined })
        }
        isReconnecting={create.isPending}
      />
    );
  }

  return (
    <div className="space-y-4">
      <CreateTunnelForm />

      {/* 筛选与搜索 */}
      <div className="flex items-center gap-2">
        <div className="flex items-center gap-1 rounded-lg border border-border p-0.5">
          {FILTERS.map((f) => (
            <button
              key={f.id}
              type="button"
              onClick={() => setFilter(f.id)}
              className={cn(
                "rounded-md px-2.5 py-1 text-xs transition-colors outline-none",
                "focus-visible:ring-2 focus-visible:ring-ring",
                filter === f.id
                  ? "bg-primary text-primary-foreground"
                  : "text-muted-foreground hover:bg-accent hover:text-foreground",
              )}
            >
              {f.label}
            </button>
          ))}
        </div>

        <div className="relative flex-1">
          <Search className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground" />
          <Input
            placeholder="搜索端口、备注或网页标题"
            value={keyword}
            onChange={(e) => setKeyword(e.target.value)}
            className="selectable pl-8"
            aria-label="搜索映射"
          />
        </div>

        {inactiveCount > 0 && (
          <Hint label="把已断开的映射移入历史，本页只留运行中的（收藏的不受影响）">
            <Button
              variant="ghost"
              size="sm"
              onClick={() => archiveInactive.mutate()}
              disabled={archiveInactive.isPending}
            >
              <Archive />
              归档已关闭
            </Button>
          </Hint>
        )}
      </div>

      {/* 标签筛选：只在确实有标签时出现，没标签的用户看不到多余控件 */}
      {tagOptions.length > 0 && (
        <div className="flex flex-wrap items-center gap-1.5">
          <Tags className="size-3.5 shrink-0 text-muted-foreground" />
          {tagOptions.map((tag) => {
            const on = activeTags.includes(tag);
            return (
              <button
                key={tag}
                type="button"
                aria-pressed={on}
                onClick={() =>
                  setActiveTags((prev) =>
                    on ? prev.filter((t) => t !== tag) : [...prev, tag],
                  )
                }
                className={cn(
                  "rounded border px-1.5 py-0.5 text-[11px] leading-none transition-colors outline-none",
                  "focus-visible:ring-2 focus-visible:ring-ring",
                  on
                    ? "border-primary bg-primary/10 text-primary"
                    : "border-dashed border-border text-muted-foreground hover:border-solid hover:text-foreground",
                )}
              >
                {tag}
              </button>
            );
          })}
          {activeTags.length > 0 && (
            <Hint label="清除标签筛选">
              <button
                type="button"
                onClick={() => setActiveTags([])}
                className="rounded px-1.5 py-0.5 text-[11px] text-muted-foreground outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
              >
                清除
              </button>
            </Hint>
          )}
        </div>
      )}

      {!hasAny ? (
        <div className="space-y-2 rounded-lg border border-dashed px-6 py-16 text-center">
          {keyword || activeTags.length > 0 ? (
            // 筛选条件要如实回报，否则用户会以为映射丢了
            <p className="text-sm text-muted-foreground">
              没有匹配
              {keyword && `「${keyword}」`}
              {keyword && activeTags.length > 0 && "、"}
              {activeTags.length > 0 && `标签「${activeTags.join("、")}」`}
              的映射。
            </p>
          ) : (
            <>
              <p className="text-sm text-muted-foreground">
                还没有映射。在上方输入本机端口即可生成公网链接。
              </p>
              {/* 给一个能直接照做的起点，而不是让用户自己想「填哪个端口」 */}
              <p className="text-xs text-muted-foreground">
                例如开发服务器常用的 <span className="font-mono">3000</span>、
                <span className="font-mono">5173</span>、
                <span className="font-mono">8080</span>。
              </p>
              <p className="text-xs text-muted-foreground">
                提示：请先确保该端口的服务已经启动。
              </p>
            </>
          )}
        </div>
      ) : (
        <div className="space-y-5">
          {sections.map(
            (s) =>
              s.items.length > 0 && (
                <section key={s.id} className="space-y-2">
                  <h2 className="flex items-center gap-1.5 text-xs font-medium text-muted-foreground">
                    {s.icon && <s.icon className="size-3.5" />}
                    {s.title}
                    <span className="tabular-nums">({s.items.length})</span>
                  </h2>
                  <div className="space-y-2">{s.items.map(renderCard)}</div>
                </section>
              ),
          )}
        </div>
      )}
    </div>
  );
}
