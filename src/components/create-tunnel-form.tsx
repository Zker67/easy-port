import { useState, type FormEvent } from "react";
import { ArrowRight, Loader2, Plus, Timer } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Hint } from "@/components/ui/tooltip";
import {
  useCreateTunnel,
  useUrlTimeoutSecs,
  useWaitingHint,
} from "@/hooks/use-tunnels";
import { cn } from "@/lib/utils";

/** 定时关闭的预设时长；"0" 表示不限时 */
export const EXPIRY_OPTIONS = [
  { value: "0", label: "不限时" },
  { value: "15", label: "15 分钟" },
  { value: "30", label: "30 分钟" },
  { value: "60", label: "1 小时" },
  { value: "180", label: "3 小时" },
  { value: "480", label: "8 小时" },
] as const;

/**
 * 创建映射的输入条。
 *
 * 视觉上是**一个**控件而不是四个并排的控件：外层一圈边框把
 * 「localhost:端口 → 时长 → 映射」裹成一条，内部用分隔线断开。
 * 之前四个等重的输入框平铺，看不出它们属于同一个动作。
 *
 * 备注不在这里收集——它属于端口而非某次创建，建好后在卡片上随时可改。
 * 少一个输入框，主路径就只剩「填端口、按回车」。
 */
export function CreateTunnelForm() {
  const [port, setPort] = useState("");
  const [expiry, setExpiry] = useState("0");
  const create = useCreateTunnel();
  const timeoutSecs = useUrlTimeoutSecs();
  const waitingHint = useWaitingHint(create.isPending, timeoutSecs);

  const portNumber = Number(port);
  const portValid =
    port !== "" && Number.isInteger(portNumber) && portNumber >= 1 && portNumber <= 65535;
  // 输入了但不合法才提示，空着不算错
  const portInvalid = port !== "" && !portValid;

  function handleSubmit(e: FormEvent) {
    e.preventDefault();
    if (!portValid || create.isPending) return;
    const minutes = Number(expiry);
    create.mutate(
      {
        port: portNumber,
        expireMinutes: minutes > 0 ? minutes : undefined,
      },
      {
        onSuccess: () => {
          setPort("");
          // 时长保持不变：连续建多条限时映射时通常用同一档
        },
      },
    );
  }

  return (
    <div className="space-y-2">
      <form
        onSubmit={handleSubmit}
        className={cn(
          "flex items-center rounded-xl border bg-card p-1.5 shadow-xs transition-colors",
          // 聚焦时整条高亮，强化「这是一个整体」
          "focus-within:border-ring focus-within:ring-[3px] focus-within:ring-ring/40",
          portInvalid && "border-destructive/60",
        )}
      >
        {/* 端口段：前缀写死 localhost:，用户只需填数字。
            这既省掉一次「要不要带 localhost」的犹豫，也点明本应用只映射本机端口。

            这里刻意不用 NumberField：它自带 ± 步进按钮和居中文本，
            端口号不是「调大调小」的量（没人靠 +1 找服务），
            步进按钮既无用又会和右侧的提交按钮凑出两个 Plus 图标。 */}
        <div className="flex flex-1 items-center gap-1 pl-2">
          <span className="shrink-0 select-none font-mono text-sm text-muted-foreground">
            localhost:
          </span>
          <input
            type="text"
            inputMode="numeric"
            autoComplete="off"
            placeholder="3000"
            value={port}
            // 只保留数字，从源头挡掉粘贴进来的杂字符
            onChange={(e) => setPort(e.target.value.replace(/\D/g, ""))}
            maxLength={5}
            autoFocus
            aria-label="本机端口"
            aria-invalid={portInvalid}
            className={cn(
              "selectable h-9 w-full min-w-0 bg-transparent font-mono text-sm",
              "outline-none placeholder:text-muted-foreground/50",
            )}
          />
        </div>

        <div className="mx-1 h-6 w-px shrink-0 bg-border" />

        {/* 时长段：默认「不限时」，多数人不会动它，因此弱化为无边框 */}
        <Hint label="到点自动断开，默认不限时">
          <Select value={expiry} onValueChange={setExpiry}>
            <SelectTrigger
              size="sm"
              className={cn(
                "w-auto gap-1.5 border-0 bg-transparent px-2 shadow-none focus-visible:ring-0",
                expiry === "0" ? "text-muted-foreground" : "text-foreground",
              )}
              aria-label="定时关闭"
            >
              <Timer
                className={cn(
                  "size-3.5",
                  expiry !== "0" && "text-primary",
                )}
              />
              <SelectValue />
            </SelectTrigger>
            <SelectContent align="end" className="min-w-28">
              {EXPIRY_OPTIONS.map((o) => (
                <SelectItem key={o.value} value={o.value}>
                  {o.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </Hint>

        <Button
          type="submit"
          size="sm"
          className="ml-1 shrink-0 gap-1.5"
          disabled={!portValid || create.isPending}
        >
          {create.isPending ? (
            <Loader2 className="animate-spin" />
          ) : portValid ? (
            // 端口合法后换成箭头：从「新增一条」变成「就映射这个端口」
            <ArrowRight />
          ) : (
            <Plus />
          )}
          {create.isPending ? "建立中" : "映射"}
        </Button>
      </form>

      {/* 等待期的进展提示：没有它，用户面对的是最长 30 秒不变的转圈 */}
      {(waitingHint || portInvalid) && (
        <p
          className={cn(
            "pl-1 text-xs",
            portInvalid ? "text-destructive" : "text-muted-foreground",
          )}
          role="status"
          aria-live="polite"
        >
          {portInvalid ? "端口需在 1 – 65535 之间" : waitingHint}
        </p>
      )}
    </div>
  );
}
