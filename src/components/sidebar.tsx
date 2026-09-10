import { useCallback, useEffect, useRef, useState } from "react";
import {
  History,
  PanelLeftClose,
  PanelLeftOpen,
  Settings,
  Waypoints,
} from "lucide-react";

import { Hint } from "@/components/ui/tooltip";
import {
  SIDEBAR_COLLAPSED_WIDTH,
  SIDEBAR_SNAP_WIDTH,
  SIDEBAR_WIDTH,
  useUiStore,
} from "@/lib/ui-store";
import { cn } from "@/lib/utils";

export type Page = "mappings" | "history" | "settings";

const ITEMS: {
  id: Page;
  label: string;
  icon: typeof Waypoints;
  hint: string;
}[] = [
  {
    id: "mappings",
    label: "映射",
    icon: Waypoints,
    hint: "工作台：建立映射，管理运行中与已断开的条目",
  },
  {
    id: "history",
    label: "历史",
    icon: History,
    hint: "全部记录（含已归档），可恢复到映射页或彻底删除",
  },
  {
    id: "settings",
    label: "设置",
    icon: Settings,
    hint: "开机自启与穿透引擎状态",
  },
];

/**
 * 侧栏导航，可收起为纯图标态。
 *
 * 见 AGENTS.md 不变量 7：不用原生控件，这里用按钮 + tablist 语义自建，
 * 焦点态与配色全部走主题 token。
 *
 * 三种收起方式并存：底部按钮、拖拽右边缘、以及拖拽后自动吸附。
 * 收起状态存在 localStorage，跨重启保留——否则每次启动都要重新收一遍。
 */
export function Sidebar({
  page,
  onChange,
  activeCount,
}: {
  page: Page;
  onChange: (page: Page) => void;
  /** 运行中的映射数，作为「映射」项的角标 */
  activeCount: number;
}) {
  const collapsed = useUiStore((s) => s.sidebarCollapsed);
  const setCollapsed = useUiStore((s) => s.setSidebarCollapsed);
  const toggle = useUiStore((s) => s.toggleSidebar);

  // 拖拽中的实时宽度；null 表示没在拖，宽度由 collapsed 决定
  const [dragWidth, setDragWidth] = useState<number | null>(null);
  const dragging = dragWidth !== null;
  // 供 pointermove 读取最新值而不必重建监听
  const latestWidth = useRef(0);

  const stopDrag = useCallback(() => {
    // 松手时按吸附阈值决定最终状态，而不是保留任意中间宽度：
    // 侧栏只有展开/收起两态，留一个 87px 的半吊子宽度没有意义。
    setCollapsed(latestWidth.current < SIDEBAR_SNAP_WIDTH);
    setDragWidth(null);
  }, [setCollapsed]);

  useEffect(() => {
    if (!dragging) return;

    // 拖拽期间锁住整个文档的选中与光标，否则快速拖动会把界面文字刷成蓝色选区，
    // 且光标会在越过内容区时变回箭头，看着像拖拽已经断了。
    const { userSelect, cursor } = document.body.style;
    document.body.style.userSelect = "none";
    document.body.style.cursor = "col-resize";

    function onMove(e: PointerEvent) {
      // 侧栏左边缘贴着窗口，鼠标的 clientX 就是宽度
      const next = Math.min(
        SIDEBAR_WIDTH,
        Math.max(SIDEBAR_COLLAPSED_WIDTH, e.clientX),
      );
      latestWidth.current = next;
      setDragWidth(next);
    }

    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", stopDrag);
    // 指针被系统取走（如切窗口）时也要收尾，否则会一直卡在拖拽态
    window.addEventListener("pointercancel", stopDrag);
    return () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", stopDrag);
      window.removeEventListener("pointercancel", stopDrag);
      document.body.style.userSelect = userSelect;
      document.body.style.cursor = cursor;
    };
  }, [dragging, stopDrag]);

  const width = dragWidth ?? (collapsed ? SIDEBAR_COLLAPSED_WIDTH : SIDEBAR_WIDTH);
  // 拖拽过程中提前按最终形态渲染，让用户看到「松手会变成什么样」
  const showLabels = dragging ? width >= SIDEBAR_SNAP_WIDTH : !collapsed;

  return (
    <div
      className="relative shrink-0"
      style={{
        width,
        // 拖拽时禁用过渡，否则跟手会有延迟感
        transition: dragging ? undefined : "width 150ms ease",
      }}
    >
      <nav
        role="tablist"
        aria-orientation="vertical"
        aria-label="主导航"
        className="flex h-full flex-col gap-1 overflow-hidden border-r border-border bg-card/40 p-2"
      >
        {ITEMS.map(({ id, label, icon: Icon, hint }) => {
          const active = page === id;
          const showBadge = id === "mappings" && activeCount > 0;
          return (
            // 角标在按钮内部，不能自己当触发器（会嵌套可交互元素），
            // 因此把数量说明并进整个导航项的提示里
            <Hint
              key={id}
              side="right"
              label={
                <span>
                  {/* 收起后 tooltip 是唯一能知道图标含义的地方，
                      因此把名称提到最前面加重，说明退居第二行 */}
                  {!showLabels && (
                    <span className="mb-0.5 block font-medium">{label}</span>
                  )}
                  {showBadge ? `${hint}（当前 ${activeCount} 条正在运行）` : hint}
                </span>
              }
            >
              <button
                type="button"
                role="tab"
                aria-selected={active}
                onClick={() => onChange(id)}
                className={cn(
                  "flex items-center gap-2.5 rounded-lg py-2 text-sm transition-colors outline-none",
                  "focus-visible:ring-2 focus-visible:ring-ring",
                  // 收起态图标居中；展开态左对齐
                  showLabels ? "px-3" : "justify-center px-0",
                  active
                    ? "bg-primary text-primary-foreground"
                    : "text-muted-foreground hover:bg-accent hover:text-foreground",
                )}
              >
                <span className="relative shrink-0">
                  <Icon className="size-4" />
                  {/* 收起后没地方放角标，改成图标右上角的小圆点，
                      否则「有几条在跑」这个信息在图标态会完全消失 */}
                  {showBadge && !showLabels && (
                    <span
                      className={cn(
                        "absolute -top-0.5 -right-0.5 size-2 rounded-full ring-2",
                        active
                          ? "bg-primary-foreground ring-primary"
                          : "bg-primary ring-card",
                      )}
                    />
                  )}
                </span>
                {showLabels && (
                  <>
                    <span className="flex-1 truncate text-left">{label}</span>
                    {showBadge && (
                      <span
                        className={cn(
                          "min-w-5 rounded-full px-1.5 py-0.5 text-center text-[11px] leading-none",
                          active
                            ? "bg-primary-foreground/20 text-primary-foreground"
                            : "bg-primary/15 text-primary",
                        )}
                      >
                        {activeCount}
                      </span>
                    )}
                  </>
                )}
              </button>
            </Hint>
          );
        })}

        {/* 收起 / 展开按钮固定在底部，不与导航项混在一起 */}
        <div className="mt-auto">
          <Hint side="right" label={collapsed ? "展开侧栏" : "收起侧栏"}>
            <button
              type="button"
              onClick={toggle}
              aria-label={collapsed ? "展开侧栏" : "收起侧栏"}
              aria-expanded={!collapsed}
              className={cn(
                "flex w-full items-center gap-2.5 rounded-lg py-2 text-sm transition-colors outline-none",
                "text-muted-foreground hover:bg-accent hover:text-foreground",
                "focus-visible:ring-2 focus-visible:ring-ring",
                showLabels ? "px-3" : "justify-center px-0",
              )}
            >
              {collapsed ? (
                <PanelLeftOpen className="size-4 shrink-0" />
              ) : (
                <PanelLeftClose className="size-4 shrink-0" />
              )}
              {showLabels && <span className="truncate">收起</span>}
            </button>
          </Hint>
        </div>
      </nav>

      {/* 拖拽把手：覆盖右边缘一条窄带。
          用 pointer 事件而非 mouse，触控板与触屏才都能拖。 */}
      <div
        role="separator"
        aria-orientation="vertical"
        aria-label="拖拽调整侧栏，向左拖到底可收起"
        onPointerDown={(e) => {
          // 只响应主键，右键菜单不该开始拖拽
          if (e.button !== 0) return;
          e.preventDefault();
          latestWidth.current = width;
          setDragWidth(width);
        }}
        onDoubleClick={toggle}
        className="group absolute inset-y-0 -right-1 w-2 cursor-col-resize"
      >
        {/* 指示线用真实元素而不是 after 伪元素：
            Tailwind 的 after: 变体需要配 content-[''] 才会生成 ::after 规则，
            漏了的话整条线一个像素都渲染不出来，且 tsc / oxlint 都不报错。 */}
        <div
          className={cn(
            "absolute inset-y-0 left-1/2 w-px -translate-x-1/2 transition-colors",
            dragging
              ? "bg-primary"
              : "bg-transparent group-hover:bg-primary/60",
          )}
        />
      </div>
    </div>
  );
}
