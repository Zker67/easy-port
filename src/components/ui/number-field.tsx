import * as React from "react";
import { Minus, Plus } from "lucide-react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

/**
 * 数字步长输入。
 *
 * 存在原因：`<input type="number">` 的原生步进箭头由浏览器绘制，
 * 不受主题 token 控制、深浅色下样式不一致、命中区域极小且各平台观感不同。
 * 这里用 `type="text"` + `inputMode="numeric"` 承载输入，步进改为自建按钮，
 * 并在 CSS 里隐藏任何残留的原生 spinner（见 index.css 的 .no-spinner）。
 */
export interface NumberFieldProps
  extends Omit<
    React.ComponentProps<"input">,
    "type" | "value" | "onChange" | "min" | "max" | "step"
  > {
  value: string;
  onValueChange: (value: string) => void;
  min?: number;
  max?: number;
  step?: number;
}

function NumberField({
  className,
  value,
  onValueChange,
  min = 1,
  max = 65535,
  step = 1,
  disabled,
  ...props
}: NumberFieldProps) {
  const current = Number(value);
  const hasValue = value !== "" && Number.isFinite(current);

  /** 步进后夹到区间内；空值时从 min 起步 */
  function nudge(delta: number) {
    const base = hasValue ? current : min - step;
    const next = Math.min(max, Math.max(min, base + delta));
    onValueChange(String(next));
  }

  const canDecrement = !disabled && (!hasValue || current > min);
  const canIncrement = !disabled && (!hasValue || current < max);

  return (
    <div
      className={cn(
        // 整体呈现为一个控件：聚焦时整框高亮，而不是只有中间的 input
        "flex h-8 items-center rounded-lg border border-input bg-transparent transition-colors",
        "focus-within:border-ring focus-within:ring-3 focus-within:ring-ring/50",
        "dark:bg-input/30",
        disabled && "pointer-events-none opacity-50",
        className,
      )}
    >
      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        className="size-7 shrink-0 rounded-md text-muted-foreground hover:text-foreground"
        onClick={() => nudge(-step)}
        disabled={!canDecrement}
        aria-label="减小"
        tabIndex={-1}
      >
        <Minus />
      </Button>

      <input
        {...props}
        // 用 text + inputMode 而非 number：避免浏览器注入原生步进箭头
        type="text"
        inputMode="numeric"
        autoComplete="off"
        value={value}
        disabled={disabled}
        onChange={(e) => {
          // 只保留数字，从源头挡掉 e / + / - / . 等 number 输入法允许的字符
          const digits = e.target.value.replace(/\D/g, "");
          onValueChange(digits);
        }}
        onKeyDown={(e) => {
          // 键盘步进是原生 number 的有用行为，这里自己实现回来
          if (e.key === "ArrowUp") {
            e.preventDefault();
            nudge(step);
          } else if (e.key === "ArrowDown") {
            e.preventDefault();
            nudge(-step);
          }
        }}
        role="spinbutton"
        aria-valuenow={hasValue ? current : undefined}
        aria-valuemin={min}
        aria-valuemax={max}
        className={cn(
          "no-spinner h-full w-full min-w-0 bg-transparent px-1 text-center text-base outline-none",
          "placeholder:text-muted-foreground md:text-sm",
        )}
      />

      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        className="size-7 shrink-0 rounded-md text-muted-foreground hover:text-foreground"
        onClick={() => nudge(step)}
        disabled={!canIncrement}
        aria-label="增大"
        tabIndex={-1}
      >
        <Plus />
      </Button>
    </div>
  );
}

export { NumberField };
