import * as React from "react";
import { Switch as SwitchPrimitive } from "radix-ui";

import { cn } from "@/lib/utils";

/**
 * 开关组件。
 *
 * 与 registry 原版的差异（勿改回去）：原版用 `data-checked:` / `data-unchecked:`
 * 和 `group-data-[size=…]/switch:` 变体，但 Radix 实际渲染的是
 * `data-state="checked" | "unchecked"`，且 `group/switch` 的具名 group 变体在
 * 本项目 Tailwind 4 配置下未生成任何 CSS——结果轨道无背景色、滑块无尺寸也不位移，
 * 视觉上只是一个黑色圆点，且看不出开关状态。
 * 这里改用标准 `data-[state=…]` 选择器并固定尺寸。
 */
function Switch({
  className,
  ...props
}: React.ComponentProps<typeof SwitchPrimitive.Root>) {
  return (
    <SwitchPrimitive.Root
      data-slot="switch"
      className={cn(
        "peer inline-flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-full border-2 border-transparent transition-colors outline-none",
        "focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background",
        "disabled:cursor-not-allowed disabled:opacity-50",
        "data-[state=checked]:bg-primary data-[state=unchecked]:bg-input",
        className,
      )}
      {...props}
    >
      <SwitchPrimitive.Thumb
        data-slot="switch-thumb"
        className={cn(
          "pointer-events-none block size-4 rounded-full bg-background shadow-lg ring-0 transition-transform",
          "data-[state=checked]:translate-x-4 data-[state=unchecked]:translate-x-0",
        )}
      />
    </SwitchPrimitive.Root>
  );
}

export { Switch };
