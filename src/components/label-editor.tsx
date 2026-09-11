import { useEffect, useRef, useState } from "react";
import { Pencil } from "lucide-react";

import { Hint } from "@/components/ui/tooltip";
import { useSetLabel } from "@/hooks/use-tunnels";
import { cn } from "@/lib/utils";

/** 备注长度上限：足够写清用途，又不至于把卡片撑变形 */
const MAX_LENGTH = 24;

/**
 * 就地编辑的备注文本。
 *
 * 备注不再只属于创建那一刻——端口是长期存在的单位，
 * 「这个端口是干什么的」随时可以补充或修正，因此每张卡片上都要能改。
 *
 * **刻意不做成胶囊/徽章**：备注是一句自由描述，标签才是分类。
 * 早先版本把它渲染成 `Badge`，与下一行的标签用了同一套形状语言，
 * 用户反馈「描述看起来就像标签一样」——两种含义不同的东西，
 * 形状必须不同。这里用普通文本 + 悬停才出现的铅笔，
 * 视觉重量低于标签，读起来像说明而不像分类项。
 *
 * 交互：点击进入编辑，Enter 保存，Esc 取消，失焦保存。
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
        placeholder="这个端口是干什么的？"
        aria-label="编辑备注"
        className={cn(
          // 编辑态只在底部出现一条线，保持「就地改一句话」的观感，
          // 不要凭空长出一个方框把行高撑开
          "selectable w-full max-w-72 border-b border-ring bg-transparent",
          "py-0.5 text-xs outline-none placeholder:text-muted-foreground/50",
          className,
        )}
      />
    );
  }

  return (
    <Hint label={label ? "点击修改备注" : "添加一句说明，记住这个端口是干什么的"}>
      <button
        type="button"
        onClick={start}
        aria-label={label ? `备注：${label}，点击修改` : "添加备注"}
        className={cn(
          "group/label inline-flex max-w-full items-center gap-1 rounded py-0.5 text-left",
          "text-xs outline-none transition-colors",
          label
            ? "text-muted-foreground hover:text-foreground"
            : // 空态更淡，但悬停即变实：它是可选项，不该和有内容的备注抢注意力
              "text-muted-foreground/50 hover:text-muted-foreground",
          "focus-visible:text-foreground focus-visible:underline focus-visible:underline-offset-2",
          className,
        )}
      >
        <span className="truncate">{label ?? "添加备注"}</span>
        {/* 铅笔平时隐形，悬停或聚焦才浮现——
            让「可编辑」这件事可被发现，又不在每张卡片上常驻一个图标 */}
        <Pencil
          className={cn(
            "size-3 shrink-0 opacity-0 transition-opacity",
            "group-hover/label:opacity-60 group-focus-visible/label:opacity-60",
          )}
        />
      </button>
    </Hint>
  );
}
