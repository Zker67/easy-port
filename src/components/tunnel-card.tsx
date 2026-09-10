import { useEffect, useRef, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import {
  Archive,
  Copy,
  ExternalLink,
  Globe,
  Loader2,
  PowerOff,
  Repeat,
  RotateCcw,
  Star,
  Timer,
  Trash2,
} from "lucide-react";
import { toast } from "sonner";

import { LabelEditor } from "@/components/label-editor";
import { PortBadge } from "@/components/port-badge";
import { TagEditor } from "@/components/tag-editor";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Hint } from "@/components/ui/tooltip";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
} from "@/components/ui/select";
import { EXPIRY_OPTIONS } from "@/components/create-tunnel-form";
import {
  useCountdown,
  useRemoveTunnel,
  useSetArchived,
  useSetAutoStart,
  useSetFavorite,
  useSetExpiry,
  useStopTunnel,
} from "@/hooks/use-tunnels";
import { isActive, type Tunnel, type TunnelStatus } from "@/lib/tunnel-api";
import { cn } from "@/lib/utils";

/** 二次确认的等待窗口：超时后按钮复原，避免一直停在确认态 */
const CONFIRM_WINDOW_MS = 3000;

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

export function TunnelCard({
  tunnel,
  autoStart,
  variant = "mapping",
  onReconnect,
  isReconnecting,
}: {
  tunnel: Tunnel;
  autoStart: boolean;
  /**
   * 卡片所在页面，决定露出哪些操作：
   * - `mapping`：完整操作（断开 / 定时 / 自动重建开关 / 归档）
   * - `history`：只读回顾 + 恢复到映射页 + 删除记录。
   *   历史页**不显示**「下次启动时自动映射」——那是映射面板的属性，
   *   在历史里设它会让人误以为归档的记录也会被自动拉起。
   */
  variant?: "mapping" | "history";
  /** 历史页的「恢复」动作，由页面注入 */
  onReconnect?: () => void;
  isReconnecting?: boolean;
}) {
  const stop = useStopTunnel();
  const remove = useRemoveTunnel();
  const setAutoStart = useSetAutoStart();
  const setArchived = useSetArchived();
  const setFavorite = useSetFavorite();
  const setExpiry = useSetExpiry();
  const isHistory = variant === "history";
  const meta = statusMeta(tunnel.status);
  const active = isActive(tunnel.status);
  const urlRef = useRef<HTMLParagraphElement>(null);
  const countdown = useCountdown(active ? tunnel.expiresAt : null);

  // 断开会让别人正在访问的链接立即失效，且新链接无法恢复为旧的，
  // 因此需要二次确认；用原地改变按钮的轻量方式，不引入 Dialog 依赖。
  const [confirmingStop, setConfirmingStop] = useState(false);

  useEffect(() => {
    if (!confirmingStop) return;
    const timer = setTimeout(() => setConfirmingStop(false), CONFIRM_WINDOW_MS);
    return () => clearTimeout(timer);
  }, [confirmingStop]);

  async function copyUrl() {
    if (!tunnel.publicUrl) return;
    try {
      await writeText(tunnel.publicUrl);
      toast.success("链接已复制");
    } catch {
      // 不谎报成功：复制失败时指向可手选的链接文本作为退路。
      toast.error("复制失败，可双击下方链接手动选中复制");
    }
  }

  async function openInBrowser() {
    if (!tunnel.publicUrl) return;
    try {
      await openUrl(tunnel.publicUrl);
    } catch {
      // 打开失败时把链接放进剪贴板兜底，而不是静默无反应。
      try {
        await writeText(tunnel.publicUrl);
        toast.error("打开浏览器失败，链接已复制到剪贴板");
      } catch {
        toast.error("打开浏览器失败，可双击下方链接手动复制");
      }
    }
  }

  /** 双击选中整条链接：链接是 truncate 显示的，手动拖选很别扭 */
  function selectUrlText() {
    const node = urlRef.current;
    if (!node) return;
    const range = document.createRange();
    range.selectNodeContents(node);
    const selection = window.getSelection();
    selection?.removeAllRanges();
    selection?.addRange(range);
  }

  function handleStopClick() {
    if (!confirmingStop) {
      setConfirmingStop(true);
      return;
    }
    setConfirmingStop(false);
    stop.mutate({ id: tunnel.id, port: tunnel.port });
  }

  return (
    <Card className="transition-colors focus-within:border-ring">
      <CardContent className="flex items-center gap-4 py-4">
        <div className="min-w-0 flex-1 space-y-1">
          <div className="flex items-center gap-2">
            <span className={cn("size-2 shrink-0 rounded-full", meta.dot)} />
            {/* 端口是这条映射的主体单位，排在最前面并做专门渲染 */}
            <PortBadge port={tunnel.port} active={active} />
            {/* 探测到的站点图标：让用户一眼认出这是哪个服务 */}
            {tunnel.site?.icon && (
              <img
                src={tunnel.site.icon}
                alt=""
                className="size-4 shrink-0 rounded-sm"
                onError={(e) => {
                  // 图标损坏时静默隐藏，不要在卡片上留一个破图标
                  e.currentTarget.style.display = "none";
                }}
              />
            )}
            {tunnel.site?.title ? (
              // 标题是 truncate 的，tooltip 用来看全称
              <Hint label={tunnel.site.title}>
                <span className="max-w-48 truncate text-sm font-medium">
                  {tunnel.site.title}
                </span>
              </Hint>
            ) : null}
            {/* 暴露状态应在视线内，而不是只写在页脚 */}
            {active && (
              <Hint label="该链接无需密码，任何人拿到即可访问">
                <Badge
                  variant="outline"
                  className="gap-1 border-status-pending/50 text-status-pending"
                >
                  <Globe className="size-3" />
                  公开
                </Badge>
              </Hint>
            )}
            {/* 定时关闭倒计时：让「还剩多久」一眼可见 */}
            {countdown && (
              <Hint
                label={`将于 ${new Date(tunnel.expiresAt!).toLocaleString()} 自动断开`}
              >
                <Badge
                  variant="outline"
                  className="gap-1 font-mono tabular-nums"
                >
                  <Timer className="size-3" />
                  {countdown}
                </Badge>
              </Hint>
            )}
            <span className="truncate text-xs text-muted-foreground">
              {meta.text}
            </span>
          </div>

          {/* 第二行：公网链接。这是这张卡片的产出物，位置固定，
              未运行时也占着这一行说明「还没有链接」，避免下方内容上下跳动。 */}
          {tunnel.publicUrl ? (
            <p
              ref={urlRef}
              onDoubleClick={selectUrlText}
              className="truncate text-xs"
            >
              {/* 单击直接在浏览器打开；双击仍可选中文本手动复制 */}
              <Hint label="点击在浏览器打开；双击可选中链接">
                <button
                  type="button"
                  onClick={() => void openInBrowser()}
                  className="selectable cursor-pointer truncate font-mono text-muted-foreground underline-offset-2 outline-none hover:text-primary hover:underline focus-visible:text-primary focus-visible:underline"
                >
                  {tunnel.publicUrl}
                </button>
              </Hint>
            </p>
          ) : (
            <p className="truncate font-mono text-xs text-muted-foreground/60">
              {active ? "正在获取链接…" : "未运行，无公网链接"}
            </p>
          )}

          {/* 第三行：备注。紧贴 URL 之下，是对「这条链接是什么」的说明 */}
          {isHistory ? (
            tunnel.label && (
              <p className="truncate text-xs text-muted-foreground">
                {tunnel.label}
              </p>
            )
          ) : (
            // 包一层 flex 让编辑器按内容宽度收缩，不横向撑满整行
            <div className="flex">
              <LabelEditor id={tunnel.id} label={tunnel.label} />
            </div>
          )}

          {/* 第四行：标签。分类信息，排在最后。
              历史页只读展示已有标签，不提供编辑——分类是工作台的事。 */}
          {isHistory ? (
            tunnel.tags.length > 0 && (
              <div className="flex flex-wrap items-center gap-1">
                {tunnel.tags.map((tag) => (
                  <span
                    key={tag}
                    className="rounded border border-dashed border-border px-1.5 py-0.5 text-[11px] leading-none text-muted-foreground"
                  >
                    {tag}
                  </span>
                ))}
              </div>
            )
          ) : (
            <TagEditor id={tunnel.id} tags={tunnel.tags} />
          )}
        </div>

        <div className="flex shrink-0 items-center gap-1">
          {/* 按钮顺序（运行中）：断开 → 定时 → 收藏 → 自动启动 → 复制 → 浏览器打开。
              左半是「对这条映射做什么」，右半是「拿这条链接做什么」，
              两组之间靠 publicUrl 分隔自然形成断点。 */}

          {/* 1. 断开：运行中最主要的操作。二次确认避免误点，
                 因为链接会立即失效且无法恢复为同一条。 */}
          {!isHistory && active && (
            <Hint
              label={
                confirmingStop
                  ? "再次点击确认断开"
                  : "断开映射。链接会立即失效，且无法恢复为同一条"
              }
            >
              <Button
                variant="destructive"
                size={confirmingStop ? "sm" : "icon-sm"}
                onClick={handleStopClick}
                onBlur={() => setConfirmingStop(false)}
                disabled={stop.isPending}
                aria-label={confirmingStop ? "确认断开映射" : "断开映射"}
              >
                {/* 与自动映射开关的图标区分：断开用 PowerOff，
                    否则同一张卡片上两个相似图标表达不同含义 */}
                {confirmingStop ? "确认断开?" : <PowerOff />}
              </Button>
            </Hint>
          )}

          {/* 未运行时，重连 / 恢复占据同一个首位——它是此时最常点的按钮 */}
          {!active && (
            <Hint
              label={
                isHistory
                  ? "重新建立该端口的映射，并回到映射页（会分配新链接）"
                  : "重新建立映射（会分配新链接）"
              }
            >
              <Button
                variant="outline"
                size="sm"
                onClick={onReconnect}
                disabled={isReconnecting}
              >
                {isReconnecting ? (
                  <Loader2 className="animate-spin" />
                ) : (
                  <RotateCcw />
                )}
                {isHistory ? "恢复" : "重连"}
              </Button>
            </Hint>
          )}

          {/* 2. 定时关闭：只对运行中的映射可设，已断开的设了没有意义。
                 value 恒为空串——当前时长无法从 expiresAt 反推回预设档，
                 这里只作为「选一个新时长」的入口，当前状态由左侧倒计时徽章表达。 */}
          {active && !isHistory && (
            <Select
              value=""
              onValueChange={(v) =>
                setExpiry.mutate({
                  id: tunnel.id,
                  minutes: v === "0" ? null : Number(v),
                })
              }
              disabled={setExpiry.isPending}
            >
              <Hint
                label={
                  tunnel.expiresAt
                    ? "更改或取消定时关闭"
                    : "设定定时关闭，到点自动断开"
                }
              >
                <SelectTrigger
                  size="sm"
                  className="w-9 justify-center px-0 [&>svg:last-child]:hidden"
                  aria-label="定时关闭"
                >
                  <Timer
                    className={cn(
                      "size-4",
                      tunnel.expiresAt
                        ? "text-primary"
                        : "text-muted-foreground",
                    )}
                  />
                </SelectTrigger>
              </Hint>
              {/* 触发器只有 36px 宽，面板需自己撑开到能放下「取消定时」 */}
              <SelectContent align="end" className="min-w-28">
                {EXPIRY_OPTIONS.map((o) => (
                  <SelectItem key={o.value} value={o.value}>
                    {o.value === "0" ? "取消定时" : o.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          )}

          {/* 3. 收藏：映射页据此置顶分区；历史页只读展示，不提供切换 */}
          {!isHistory && (
            <Hint
              label={tunnel.favorite ? "取消收藏" : "收藏，置顶到收藏分区"}
            >
              <Button
                variant="ghost"
                size="icon-sm"
                onClick={() =>
                  setFavorite.mutate({
                    id: tunnel.id,
                    favorite: !tunnel.favorite,
                  })
                }
                disabled={setFavorite.isPending}
                aria-pressed={tunnel.favorite}
                aria-label={tunnel.favorite ? "取消收藏" : "收藏"}
              >
                <Star
                  className={cn(
                    tunnel.favorite && "fill-status-pending text-status-pending",
                  )}
                />
              </Button>
            </Hint>
          )}

          {/* 4. 开机自动映射开关。只属于映射面板：
                 历史里的记录是回顾，不该能配置自动拉起。

                 图标用 Repeat 而非 Power——后者太像「启动/断开」，与旁边的
                 断开按钮撞语义；也不用日历/闹钟类图标，那会和左边的定时关闭撞。
                 Repeat 表达的是「每次启动都重来一遍」，正是这个开关的含义。

                 语义完全由 tooltip 与 aria-label 承载，因此两者都必须
                 写全「开 / 关」当前态，不能只写动作。 */}
          {!isHistory && (
            <Hint
              label={
                autoStart
                  ? "已开启：下次启动 Easy Port 时自动映射此端口。会分配一条新链接，旧链接无法恢复。点击关闭。"
                  : "已关闭：下次启动 Easy Port 时不会自动映射此端口。点击开启。"
              }
            >
              <Button
                variant={autoStart ? "secondary" : "ghost"}
                size="icon-sm"
                onClick={() =>
                  setAutoStart.mutate({ id: tunnel.id, enabled: !autoStart })
                }
                disabled={setAutoStart.isPending}
                aria-pressed={autoStart}
                aria-label={
                  autoStart
                    ? "已开启下次启动时自动映射，点击关闭"
                    : "已关闭下次启动时自动映射，点击开启"
                }
              >
                <Repeat className={cn(autoStart && "text-primary")} />
              </Button>
            </Hint>
          )}

          {/* 5 / 6. 复制与浏览器打开：对「链接」的操作，排在对「映射」的操作之后 */}
          {tunnel.publicUrl && (
            <>
              <Hint label="复制链接到剪贴板">
                <Button
                  variant="outline"
                  size="icon-sm"
                  onClick={() => void copyUrl()}
                  aria-label="复制链接"
                >
                  <Copy />
                </Button>
              </Hint>
              <Hint label="在默认浏览器中打开">
                <Button
                  variant="outline"
                  size="icon-sm"
                  onClick={() => void openInBrowser()}
                  aria-label="在浏览器打开"
                >
                  <ExternalLink />
                </Button>
              </Hint>
            </>
          )}

          {/* 收尾的破坏性 / 收纳操作，与上面的常用操作拉开距离 */}
          {isHistory && (
            <Hint label="彻底删除该记录，不可撤销">
              <Button
                variant="ghost"
                size="icon-sm"
                onClick={() =>
                  remove.mutate({ id: tunnel.id, port: tunnel.port })
                }
                disabled={remove.isPending}
                aria-label="彻底删除该记录"
              >
                <Trash2 />
              </Button>
            </Hint>
          )}

          {!isHistory && !active && (
            <Hint label="归档：从映射列表移走，记录仍保留在历史中">
              <Button
                variant="ghost"
                size="icon-sm"
                onClick={() =>
                  setArchived.mutate({ id: tunnel.id, archived: true })
                }
                disabled={setArchived.isPending}
                aria-label="归档"
              >
                <Archive />
              </Button>
            </Hint>
          )}
        </div>
      </CardContent>
    </Card>
  );
}
