import { Toaster as Sonner, type ToasterProps } from "sonner";

/**
 * 桌面端 Toaster。
 * 不接 next-themes：本应用跟随系统深色模式，由 CSS 的 .dark class 控制。
 */
function Toaster(props: ToasterProps) {
  return (
    <Sonner
      className="toaster group"
      position="bottom-right"
      style={
        {
          "--normal-bg": "var(--popover)",
          "--normal-text": "var(--popover-foreground)",
          "--normal-border": "var(--border)",
        } as React.CSSProperties
      }
      {...props}
    />
  );
}

export { Toaster };
