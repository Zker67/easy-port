import * as React from "react";
import { Tooltip as TooltipPrimitive } from "radix-ui";

import { cn } from "@/lib/utils";

/**
 * 自建 tooltip，替代浏览器原生的 `title` 属性（见 AGENTS.md 不变量 7 的同类理由）。
 *
 * 原生 `title` 的问题：延迟约 1 秒且不可调、样式完全不受主题控制、
 * 深浅色下观感不一致、位置由浏览器决定、无法换行排版长文案。
 *
 * 取自 shadcn registry，但按本项目环境改了三处（registry 源码不保证可直接用）：
 * 1. `data-open:` / `data-closed:` 改为标准 `data-[state=…]`——
 *    Radix 渲染的是 `data-state="open"`，原写法生成不出任何 CSS，动画会完全失效。
 *    与 `switch.tsx` 踩过的是同一个坑，`tsc` 和 `oxlint` 都发现不了。
 * 2. 删掉 Next.js 的 `"use client"` 指令。
 * 3. 去掉 `has-data-[slot=kbd]` 等本项目用不到的 kbd 相关变体。
 * 4. 不渲染箭头（`TooltipPrimitive.Arrow`），理由见 `TooltipContent` 内注释。
 */
function TooltipProvider({
  delayDuration = 300,
  ...props
}: React.ComponentProps<typeof TooltipPrimitive.Provider>) {
  return (
    <TooltipPrimitive.Provider
      data-slot="tooltip-provider"
      delayDuration={delayDuration}
      {...props}
    />
  );
}

function Tooltip({
  ...props
}: React.ComponentProps<typeof TooltipPrimitive.Root>) {
  return <TooltipPrimitive.Root data-slot="tooltip" {...props} />;
}

function TooltipTrigger({
  ...props
}: React.ComponentProps<typeof TooltipPrimitive.Trigger>) {
  return <TooltipPrimitive.Trigger data-slot="tooltip-trigger" {...props} />;
}

function TooltipContent({
  className,
  // 无箭头，间距略收紧一点，气泡更贴近触发器
  sideOffset = 5,
  collisionPadding = 8,
  children,
  ...props
}: React.ComponentProps<typeof TooltipPrimitive.Content>) {
  return (
    <TooltipPrimitive.Portal>
      <TooltipPrimitive.Content
        data-slot="tooltip-content"
        sideOffset={sideOffset}
        // 贴近窗口边缘时留出 8px 余量。碰撞规避本身默认就开着，
        // 但默认 padding 为 0，提示会紧贴边框，观感上像被截断
        collisionPadding={collisionPadding}
        className={cn(
          "z-50 w-max origin-(--radix-tooltip-content-transform-origin)",
          // 用可用空间算上限：Radix 会把剩余宽度写进这个变量，
          // 靠边时才收窄，居中时能一行放下，不再固定 max-w-64 硬折行
          "max-w-[min(20rem,var(--radix-tooltip-content-available-width))]",
          "rounded-md bg-popover px-2.5 py-1.5 text-xs text-popover-foreground",
          "border border-border shadow-md",
          // 需要折行时让各行长度均衡，不出现「最后一行只剩两个字」
          "text-pretty",
          "data-[side=bottom]:slide-in-from-top-1 data-[side=left]:slide-in-from-right-1",
          "data-[side=right]:slide-in-from-left-1 data-[side=top]:slide-in-from-bottom-1",
          "data-[state=delayed-open]:animate-in data-[state=delayed-open]:fade-in-0 data-[state=delayed-open]:zoom-in-95",
          "data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95",
          "data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95",
          className,
        )}
        {...props}
      >
        {/* 刻意不渲染 TooltipPrimitive.Arrow：
            它是个旋转 45° 的方块，要跟气泡的描边严丝合缝地拼上，
            需要精确抵消旋转后的位移并只保留两条边——很难在各个 side 下都对齐，
            实测会出现一个位置不对的小三角。气泡本身已经贴着触发器，
            没有箭头也不会指代不清，索性去掉。 */}
        {children}
      </TooltipPrimitive.Content>
    </TooltipPrimitive.Portal>
  );
}

/**
 * 便捷包装：`<Hint label="说明"><Button/></Hint>`。
 *
 * 存在原因：全应用有十几处提示，每处都写四层
 * `Tooltip / TooltipTrigger asChild / TooltipContent` 太吵，
 * 也容易漏掉 `asChild` 导致按钮被套进一个多余的 button。
 *
 * **不要用它替代 `aria-label`**：tooltip 只在悬停/聚焦时出现，
 * 键盘朗读与触屏用户依赖的仍是 `aria-label`，两者各司其职。
 */
function Hint({
  label,
  children,
  side = "top",
  align = "center",
  ...props
}: {
  label: React.ReactNode;
  children: React.ReactNode;
} & Omit<React.ComponentProps<typeof TooltipPrimitive.Content>, "content">) {
  if (!label) return <>{children}</>;
  return (
    <Tooltip>
      <TooltipTrigger asChild>{children}</TooltipTrigger>
      <TooltipContent side={side} align={align} {...props}>
        {label}
      </TooltipContent>
    </Tooltip>
  );
}

export { Hint, Tooltip, TooltipContent, TooltipProvider, TooltipTrigger };
