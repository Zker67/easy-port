import { cn } from "@/lib/utils";

/**
 * 端口号的专用渲染。
 *
 * 端口是这个应用里的**主体单位**——一条映射就是「某个端口对外的一扇门」，
 * 因此它值得一个能一眼锁定的样式，而不是混在文本里的 `localhost:3000`。
 *
 * 刻意只显示数字，不带 `localhost:` 也不带 `:` 前缀：
 * 本应用映射的永远是本机端口，这些前缀对每一条都相同，
 * 不携带任何信息，只占位置。胶囊样式本身已经表明「这是端口」。
 */
export function PortBadge({
  port,
  active,
  className,
}: {
  port: number;
  /** 运行中的端口用主题色实心，突出「这扇门开着」 */
  active?: boolean;
  className?: string;
}) {
  return (
    <span
      className={cn(
        "inline-flex shrink-0 items-center rounded-md border px-2 py-0.5",
        "font-mono text-base leading-none font-semibold tabular-nums",
        active
          ? "border-primary/30 bg-primary/10 text-primary"
          : "border-border bg-muted/60 text-muted-foreground",
        className,
      )}
    >
      {port}
    </span>
  );
}
