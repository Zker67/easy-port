import { useEffect, useRef, useState } from "react";
import { Plus, X } from "lucide-react";

import { Hint } from "@/components/ui/tooltip";
import { useAllTags, useSetTags } from "@/hooks/use-tunnels";
import { cn } from "@/lib/utils";

/** 与 Rust 侧 MAX_TAGS_PER_TUNNEL 对应；超出部分会被后端截断 */
const MAX_TAGS = 5;
const MAX_LENGTH = 12;

/**
 * 标签编辑器：展示已有标签，并提供「加标签」入口。
 *
 * 与备注（`label-editor.tsx`）的分工：
 * 备注是每个端口一条的自由文本，标签是可跨端口复用的人工分类，用于筛选。
 *
 * 「随用随建」：没有独立的标签管理页，输入即创建；
 * 输入时给出已有标签的建议，避免同一分类被写成好几种写法。
 */
export function TagEditor({
  id,
  tags,
  className,
}: {
  id: string;
  tags: string[];
  className?: string;
}) {
  const setTags = useSetTags();
  const allTags = useAllTags();
  const [adding, setAdding] = useState(false);
  const [draft, setDraft] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const committed = useRef(false);

  useEffect(() => {
    if (adding) inputRef.current?.focus();
  }, [adding]);

  // 建议里排掉本条已有的，避免点了没反应
  const suggestions = (allTags.data ?? []).filter(
    (t) => !tags.some((own) => own.toLowerCase() === t.toLowerCase()),
  );

  function add(raw: string) {
    const next = raw.trim();
    setAdding(false);
    setDraft("");
    if (!next) return;
    // 已有同名（忽略大小写）就不重复添加
    if (tags.some((t) => t.toLowerCase() === next.toLowerCase())) return;
    setTags.mutate({ id, tags: [...tags, next] });
  }

  function remove(tag: string) {
    setTags.mutate({ id, tags: tags.filter((t) => t !== tag) });
  }

  function commit() {
    if (committed.current) return;
    committed.current = true;
    add(draft);
  }

  const full = tags.length >= MAX_TAGS;

  return (
    <div className={cn("flex flex-wrap items-center gap-1", className)}>
      {tags.map((tag) => (
        <span
          key={tag}
          className={cn(
            // 实心药丸：与备注那行的纯文本拉开形状差异。
            // 标签是「一类东西」，视觉上就该是可数、成组、边界清晰的色块。
            "group/tag inline-flex items-center gap-0.5 rounded-full",
            "bg-primary/10 py-0.5 pl-2 pr-1 text-[11px] leading-none font-medium",
            "text-primary/90 transition-colors hover:bg-primary/15",
          )}
        >
          {tag}
          <Hint label={`移除标签「${tag}」`}>
            <button
              type="button"
              onClick={() => remove(tag)}
              disabled={setTags.isPending}
              aria-label={`移除标签 ${tag}`}
              className={cn(
                // × 平时半隐，悬停整枚标签才显形，避免一排标签全是删除号
                "rounded-full p-0.5 opacity-40 outline-none transition-all",
                "group-hover/tag:opacity-100",
                "hover:bg-destructive/20 hover:text-destructive",
                "focus-visible:opacity-100 focus-visible:ring-1 focus-visible:ring-ring",
              )}
            >
              <X className="size-2.5" />
            </button>
          </Hint>
        </span>
      ))}

      {adding ? (
        <span className="relative">
          <input
            ref={inputRef}
            value={draft}
            maxLength={MAX_LENGTH}
            // 已有标签作为输入建议，减少同一分类的多种写法
            list={`tags-${id}`}
            onChange={(e) => setDraft(e.target.value)}
            onBlur={commit}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                commit();
              } else if (e.key === "Escape") {
                e.preventDefault();
                committed.current = true;
                setAdding(false);
                setDraft("");
              }
            }}
            placeholder="标签名"
            aria-label="新标签名称"
            className={cn(
              "selectable h-[22px] w-24 rounded-full border border-ring bg-background px-2",
              "text-[11px] outline-none",
            )}
          />
          {/* datalist 是浏览器建议列表，不是可交互控件，
              不受「不用原生控件」约束（它没有可见的原生外观） */}
          <datalist id={`tags-${id}`}>
            {suggestions.map((t) => (
              <option key={t} value={t} />
            ))}
          </datalist>
        </span>
      ) : (
        !full && (
          <Hint
            label={
              suggestions.length > 0
                ? `添加标签用于分类筛选。已有：${suggestions.join("、")}`
                : "添加标签用于分类筛选，输入即创建"
            }
          >
            <button
              type="button"
              onClick={() => {
                committed.current = false;
                setAdding(true);
              }}
              aria-label="添加标签"
              className={cn(
                "inline-flex items-center gap-0.5 rounded-full py-0.5 pl-1.5 pr-2 text-[11px]",
                // 虚线勾出「这里还能再放一枚」的空位，形状与实心标签一致，
                // 只是没有填充——不能只靠淡色，那样在深色主题下几乎看不见
                "border border-dashed border-border text-muted-foreground",
                "transition-colors outline-none",
                "hover:border-solid hover:border-primary/40 hover:bg-primary/10 hover:text-primary",
                "focus-visible:ring-2 focus-visible:ring-ring focus-visible:text-foreground",
              )}
            >
              <Plus className="size-2.5" />
              标签
            </button>
          </Hint>
        )
      )}
    </div>
  );
}
