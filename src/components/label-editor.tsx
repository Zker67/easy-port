import { useEffect, useRef, useState } from "react";
import { Tag } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Hint } from "@/components/ui/tooltip";
import { useSetLabel } from "@/hooks/use-tunnels";
import { cn } from "@/lib/utils";

/** 备注长度上限：足够写清用途，又不至于把卡片撑变形 */
const MAX_LENGTH = 24;

/**
 * 就地编辑的备注标签。
 *
 * 备注不再只属于创建那一刻——端口是长期存在的单位，
 * 「这个端口是干什么的」随时可以补充或修正，因此每张卡片上都要能改。
 *
 * 交互：点击进入编辑，Enter 保存，Esc 取消，失焦保存。
 * 没有备注时显示一个淡的「加备注」入口，不占额外空间。
 */
export function LabelEditor({
  id,
  label,
  className,
}: {
  id: string;
  label: string | null;
  className?: string;
}) {
  const setLabel = useSetLabel();
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(label ?? "");
  const inputRef = useRef<HTMLInputElement>(null);
  // 保存与取消都可能触发 blur，用它避免同一次编辑被提交两次
  const committed = useRef(false);

  useEffect(() => {
    if (editing) {
      inputRef.current?.focus();
      inputRef.current?.select();
    }
  }, [editing]);

  function start() {
    // 每次进入编辑都从当前值起步，而不是沿用上次的草稿
    setDraft(label ?? "");
    committed.current = false;
    setEditing(true);
  }

  function commit() {
    if (committed.current) return;
    committed.current = true;
    setEditing(false);

    const next = draft.trim();
    // 没变就不打扰后端，也不触发一次无谓的列表刷新
    if (next === (label ?? "")) return;
    setLabel.mutate({ id, label: next || null });
  }

  function cancel() {
    committed.current = true;
    setEditing(false);
    setDraft(label ?? "");
  }

  if (editing) {
    return (
      <input
        ref={inputRef}
        value={draft}
        maxLength={MAX_LENGTH}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            commit();
          } else if (e.key === "Escape") {
            e.preventDefault();
            cancel();
          }
        }}
        placeholder="备注"
        aria-label="编辑备注"
        className={cn(
          "selectable h-6 w-28 rounded-md border border-ring bg-background px-1.5 text-xs",
          "outline-none",
          className,
        )}
      />
    );
  }

  if (!label) {
    return (
      <Hint label="添加备注，用来记住这个端口是干什么的">
        <button
          type="button"
          onClick={start}
          aria-label="添加备注"
          className={cn(
            "inline-flex items-center gap-1 rounded-md px-1.5 py-0.5 text-xs",
            // 用虚线边框标出这是个可点的空位。
            // 曾经只用 text-muted-foreground/50 而无边框，结果深色主题下几乎看不见，
            // 用户反馈「找不到编辑入口」——入口再克制也必须先能被发现。
            "border border-dashed border-border text-muted-foreground",
            "transition-colors outline-none",
            "hover:border-solid hover:border-primary/50 hover:bg-accent hover:text-foreground",
            "focus-visible:ring-2 focus-visible:ring-ring focus-visible:text-foreground",
            className,
          )}
        >
          <Tag className="size-3" />
          备注
        </button>
      </Hint>
    );
  }

  return (
    <Hint label="点击修改备注">
      <button
        type="button"
        onClick={start}
        aria-label={`备注：${label}，点击修改`}
        className={cn(
          "rounded-md outline-none focus-visible:ring-2 focus-visible:ring-ring",
          className,
        )}
      >
        <Badge
          variant="secondary"
          className="max-w-32 cursor-pointer truncate hover:bg-accent"
        >
          {label}
        </Badge>
      </button>
    </Hint>
  );
}
