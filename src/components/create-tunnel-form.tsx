import { useState, type FormEvent } from "react";
import { Loader2, Plus } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { NumberField } from "@/components/ui/number-field";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  useCreateTunnel,
  useUrlTimeoutSecs,
  useWaitingHint,
} from "@/hooks/use-tunnels";

/** 定时关闭的预设时长；"0" 表示不限时 */
export const EXPIRY_OPTIONS = [
  { value: "0", label: "不限时" },
  { value: "15", label: "15 分钟" },
  { value: "30", label: "30 分钟" },
  { value: "60", label: "1 小时" },
  { value: "180", label: "3 小时" },
  { value: "480", label: "8 小时" },
] as const;

export function CreateTunnelForm() {
  const [port, setPort] = useState("");
  const [label, setLabel] = useState("");
  const [expiry, setExpiry] = useState("0");
  const create = useCreateTunnel();
  const timeoutSecs = useUrlTimeoutSecs();
  const waitingHint = useWaitingHint(create.isPending, timeoutSecs);

  const portNumber = Number(port);
  const portValid =
    port !== "" && Number.isInteger(portNumber) && portNumber >= 1 && portNumber <= 65535;

  function handleSubmit(e: FormEvent) {
    e.preventDefault();
    if (!portValid || create.isPending) return;
    const minutes = Number(expiry);
    create.mutate(
      {
        port: portNumber,
        label: label.trim() || undefined,
        expireMinutes: minutes > 0 ? minutes : undefined,
      },
      {
        onSuccess: () => {
          setPort("");
          setLabel("");
          // 时长保持不变：连续建多条限时映射时通常用同一档
        },
      },
    );
  }

  return (
    <div className="space-y-2">
      <form onSubmit={handleSubmit} className="flex items-center gap-2">
        <NumberField
          placeholder="本机端口"
          value={port}
          onValueChange={setPort}
          className="selectable w-44"
          min={1}
          max={65535}
          aria-label="本机端口"
        />
        {/* 备注不是必须在这里定：建好后点卡片上的标签随时能改 */}
        <Input
          placeholder="备注（可选，之后也能改）"
          value={label}
          onChange={(e) => setLabel(e.target.value)}
          className="selectable flex-1"
          aria-label="备注"
        />
        <Select value={expiry} onValueChange={setExpiry}>
          <SelectTrigger className="w-28" aria-label="定时关闭">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {EXPIRY_OPTIONS.map((o) => (
              <SelectItem key={o.value} value={o.value}>
                {o.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <Button type="submit" disabled={!portValid || create.isPending}>
          {create.isPending ? <Loader2 className="animate-spin" /> : <Plus />}
          {create.isPending ? "建立中" : "映射"}
        </Button>
      </form>

      {/* 等待期的进展提示：没有它，用户面对的是最长 30 秒不变的转圈 */}
      {waitingHint && (
        <p
          className="pl-1 text-xs text-muted-foreground"
          role="status"
          aria-live="polite"
        >
          {waitingHint}
        </p>
      )}
    </div>
  );
}
