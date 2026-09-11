import { cn } from "@/lib/utils";

/**
 * 信息页的键值行：左列定宽标签，右列自适应内容。
 *
 * 引擎页与设置页共用，保证两页的左列对齐一致——
 * 各写一份必然在某次改动后错开一两个像素。
 *
 * 配 `<dl className="divide-y">` 使用，分隔线由父级统一提供。
 */
export function InfoRow({
  label,
  children,
  mono,
}: {
  label: string;
  children: React.ReactNode;
  /** 内容是路径 / 文件名等需要等宽与可选中的场合 */
  mono?: boolean;
}) {
  return (
    <div className="flex gap-3 py-2">
      <dt className="w-20 shrink-0 text-xs text-muted-foreground">{label}</dt>
      <dd
        className={cn(
          "min-w-0 flex-1 text-xs",
          mono && "selectable break-all font-mono",
        )}
      >
        {children}
      </dd>
    </div>
  );
}

/**
 * 一行里「主值 + 补充说明」的排版。
 *
 * 用 flex + gap 而不是在文本里插空格：全角空格在不同字体下宽度不一致，
 * 且窄窗口时无法换行。
 */
export function RowNote({ children }: { children: React.ReactNode }) {
  return (
    <span className="flex flex-wrap items-baseline gap-x-2 gap-y-0.5">
      {children}
    </span>
  );
}
