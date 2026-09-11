import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Cable, Minimize2, Minus, Square, X } from "lucide-react";

import { Hint } from "@/components/ui/tooltip";
import { useAppVersion } from "@/hooks/use-app-version";
import { cn } from "@/lib/utils";

const appWindow = getCurrentWindow();

/**
 * 自建标题栏。
 *
 * 见 AGENTS.md 不变量 7：不使用原生控件。窗口已设 `decorations: false`，
 * 系统标题栏不再绘制，拖拽、最小化、最大化、关闭全部由本组件承担。
 *
 * 拖拽依赖 `data-tauri-drag-region`：该属性由 Tauri 在 WebView 层识别，
 * 必须落在实际可点击的空白区域上，且不能被子元素完全覆盖。
 */
export function Titlebar({ children }: { children?: React.ReactNode }) {
  const [maximized, setMaximized] = useState(false);
  const version = useAppVersion();

  useEffect(() => {
    let disposed = false;

    // 初始状态 + 跟随窗口尺寸变化（双击标题栏、系统快捷键都会改变它）
    void appWindow.isMaximized().then((v) => {
      if (!disposed) setMaximized(v);
    });

    const unlisten = appWindow.onResized(() => {
      void appWindow.isMaximized().then((v) => {
        if (!disposed) setMaximized(v);
      });
    });

    return () => {
      disposed = true;
      void unlisten.then((fn) => fn());
    };
  }, []);

  return (
    <header
      data-tauri-drag-region
      className="flex h-10 shrink-0 select-none items-center justify-between border-b border-border bg-background pl-3"
    >
      {/* 左侧标识：也要能拖拽，否则可拖区域太窄 */}
      <div
        data-tauri-drag-region
        className="flex items-center gap-2 text-sm font-semibold"
      >
        <Cable className="size-4 text-primary" />
        Easy Port
        {/* 版本标签：弱化处理，不与应用名争视觉重量。
            取不到版本时整块不渲染，不留一个空壳。 */}
        {version && (
          <span className="rounded bg-muted px-1.5 py-px font-mono text-[10px] font-normal leading-normal text-muted-foreground">
            v{version}
          </span>
        )}
      </div>

      {/* 中间留给计数等状态，同样可拖拽 */}
      <div
        data-tauri-drag-region
        className="flex flex-1 items-center justify-end gap-2 px-3"
      >
        {children}
      </div>

      {/* 窗口控制：不可拖拽，否则点击会被判定为拖窗 */}
      <div className="flex items-center">
        <TitlebarButton onClick={() => void appWindow.minimize()} label="最小化">
          <Minus className="size-4" />
        </TitlebarButton>

        <TitlebarButton
          onClick={() => void appWindow.toggleMaximize()}
          label={maximized ? "向下还原" : "最大化"}
        >
          {maximized ? (
            <Minimize2 className="size-3.5" />
          ) : (
            <Square className="size-3.5" />
          )}
        </TitlebarButton>

        <TitlebarButton
          onClick={() => void appWindow.close()}
          label="关闭"
          danger
        >
          <X className="size-4" />
        </TitlebarButton>
      </div>
    </header>
  );
}

function TitlebarButton({
  onClick,
  label,
  danger,
  children,
}: {
  onClick: () => void;
  label: string;
  danger?: boolean;
  children: React.ReactNode;
}) {
  return (
    // 标题栏按钮贴着窗口顶边，提示往下走才不会被窗口边缘裁掉
    <Hint label={label} side="bottom" sideOffset={2}>
      <button
        type="button"
        onClick={onClick}
        aria-label={label}
        className={cn(
          // 尺寸对齐 Windows 标题栏习惯：整块高度、无圆角、hover 才出底色
          "inline-flex h-10 w-12 items-center justify-center text-muted-foreground transition-colors",
          "outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring",
          danger
            ? "hover:bg-destructive hover:text-white"
            : "hover:bg-accent hover:text-foreground",
        )}
      >
        {children}
      </button>
    </Hint>
  );
}
