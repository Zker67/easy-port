import { useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import {
  Copy,
  ExternalLink,
  KeyRound,
  Loader2,
  Power,
  PowerOff,
  RefreshCw,
  ShieldAlert,
} from "lucide-react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { NumberField } from "@/components/ui/number-field";
import { Switch } from "@/components/ui/switch";
import { Hint } from "@/components/ui/tooltip";
import {
  useRegenerateWebToken,
  useSetWebAutoStart,
  useSetWebLabel,
  useSetWebPort,
  useStartWebConsole,
  useStopWebConsole,
  useWebConsole,
} from "@/hooks/use-web-console";
import type { WebConsoleView } from "@/lib/tunnel-api";
import { cn } from "@/lib/utils";

/** 二次确认的等待窗口，与卡片上的断开保持一致 */
const CONFIRM_WINDOW_MS = 3000;

/**
 * Web 远程控制台。
 *
 * 与其他页面的根本区别：这里暴露的是**应用自己的控制面**，
 * 而不是用户的某个服务。因此风险说明常驻置顶、不可折叠。
 */
export function WebPage() {
  const query = useWebConsole();
  const view = query.data;

  if (!view) {
    return (
      <div className="flex h-40 items-center justify-center">
        <Loader2 className="size-5 animate-spin text-muted-foreground" />
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <RiskNotice />
      <TokenSection view={view} />
      <ConfigSection view={view} />
      <RunSection view={view} />
    </div>
  );
}

/**
 * 常驻风险说明。
 *
 * **刻意不做成可关闭的**：这个功能的风险高于普通映射，
 * 每次开启前都值得再看一眼。
 */
function RiskNotice() {
  return (
    <div className="flex gap-2.5 rounded-lg border border-status-pending/40 bg-status-pending/5 px-3 py-2.5">
      <ShieldAlert className="mt-0.5 size-4 shrink-0 text-status-pending" />
      <div className="space-y-1 text-xs">
        <p className="font-medium text-foreground">
          开启后，任何拿到链接<span className="text-status-pending">并知道 token</span>
          的人都能远程开关你的端口映射。
        </p>
        <p className="text-muted-foreground">
          请把 token 当作密码保管，不要和链接一起发给别人。
          手机端只能开关已有映射，不能新建或修改配置。
        </p>
      </div>
    </div>
  );
}

function TokenSection({ view }: { view: WebConsoleView }) {
  const regenerate = useRegenerateWebToken();
  const [plaintext, setPlaintext] = useState<string | null>(null);
  const [confirming, setConfirming] = useState(false);

  useEffect(() => {
    if (!confirming) return;
    const timer = setTimeout(() => setConfirming(false), CONFIRM_WINDOW_MS);
    return () => clearTimeout(timer);
  }, [confirming]);

  function generate() {
    regenerate.mutate(undefined, {
      // 明文只在这一刻拿得到，必须立刻交给用户
      onSuccess: (token) => setPlaintext(token),
    });
    setConfirming(false);
  }

  async function copyToken() {
    if (!plaintext) return;
    try {
      await writeText(plaintext);
      toast.success("token 已复制");
    } catch {
      toast.error("复制失败，请手动选中上方 token");
    }
  }

  return (
    <Card>
      <CardContent className="space-y-3 py-4">
        <div className="flex items-center gap-2">
          <KeyRound className="size-4 text-muted-foreground" />
          <h2 className="text-sm font-medium">访问 token</h2>
        </div>

        {plaintext ? (
          <div className="space-y-2">
            <div className="flex items-center gap-2">
              <code className="selectable flex-1 truncate rounded-md border border-primary/40 bg-primary/5 px-2 py-1.5 font-mono text-xs">
                {plaintext}
              </code>
              <Hint label="复制 token">
                <Button variant="outline" size="icon-sm" onClick={() => void copyToken()}>
                  <Copy />
                </Button>
              </Hint>
            </div>
            <p className="text-xs text-status-pending">
              只显示这一次，请立即保存。关闭本页或刷新后将无法再查看，只能重新生成。
            </p>
          </div>
        ) : view.hasToken ? (
          <div className="flex items-center gap-2">
            <span className="flex-1 font-mono text-xs text-muted-foreground">
              已设置 ••••••••••••
            </span>
            {/* 不提供「查看」入口：只存哈希，不可逆，界面上也不该假装能看 */}
            <Button
              variant="outline"
              size="sm"
              onClick={() => (confirming ? generate() : setConfirming(true))}
              onBlur={() => setConfirming(false)}
              disabled={regenerate.isPending}
            >
              {regenerate.isPending ? (
                <Loader2 className="animate-spin" />
              ) : (
                <RefreshCw />
              )}
              {confirming ? "确认重新生成?" : "重新生成"}
            </Button>
          </div>
        ) : (
          <div className="flex items-center gap-2">
            <span className="flex-1 text-xs text-muted-foreground">
              尚未设置 token，开启控制台前必须先生成。
            </span>
            <Button size="sm" onClick={generate} disabled={regenerate.isPending}>
              {regenerate.isPending ? (
                <Loader2 className="animate-spin" />
              ) : (
                <KeyRound />
              )}
              生成 token
            </Button>
          </div>
        )}

        {confirming && view.hasToken && (
          <p className="text-xs text-muted-foreground">
            重新生成会让旧 token 立即失效，已登录的设备会被踢下线。
          </p>
        )}
      </CardContent>
    </Card>
  );
}

function ConfigSection({ view }: { view: WebConsoleView }) {
  const setPort = useSetWebPort();
  const setLabel = useSetWebLabel();
  const setAutoStart = useSetWebAutoStart();
  const [port, setPortDraft] = useState(String(view.port));
  const [label, setLabelDraft] = useState(view.label ?? "");
  const running =
    view.status.kind === "running" || view.status.kind === "starting";

  // 外部变化（例如另一处改了配置）时同步草稿
  useEffect(() => setPortDraft(String(view.port)), [view.port]);
  useEffect(() => setLabelDraft(view.label ?? ""), [view.label]);

  function commitPort() {
    const n = Number(port);
    if (!Number.isInteger(n) || n < 1 || n > 65535) {
      setPortDraft(String(view.port));
      return;
    }
    if (n !== view.port) setPort.mutate(n);
  }

  return (
    <Card>
      <CardContent className="space-y-3 py-4">
        <div className="flex items-center gap-3">
          <Label className="w-20 shrink-0 text-xs text-muted-foreground">
            监听端口
          </Label>
          <NumberField
            value={port}
            onValueChange={setPortDraft}
            onBlur={commitPort}
            min={1}
            max={65535}
            disabled={running}
            className="selectable w-36"
            aria-label="控制台监听端口"
          />
          {running && (
            <span className="text-xs text-muted-foreground">
              运行中不可修改
            </span>
          )}
        </div>

        <div className="flex items-center gap-3">
          <Label className="w-20 shrink-0 text-xs text-muted-foreground">
            备注
          </Label>
          <Input
            value={label}
            onChange={(e) => setLabelDraft(e.target.value)}
            onBlur={() => {
              const next = label.trim();
              if (next !== (view.label ?? "")) setLabel.mutate(next || null);
            }}
            maxLength={24}
            placeholder="例如：家里的电脑"
            className="selectable flex-1"
            aria-label="控制台备注"
          />
        </div>

        <div className="flex items-center gap-3">
          <Label
            htmlFor="web-auto-start"
            className="w-20 shrink-0 cursor-pointer text-xs text-muted-foreground"
          >
            开机自启
          </Label>
          <Switch
            id="web-auto-start"
            checked={view.autoStart}
            disabled={setAutoStart.isPending}
            onCheckedChange={(enabled) => {
              if (enabled) {
                // 开机即在公网挂一个控制入口，值得一次明确警告
                toast.warning("已开启：今后开机即会在公网暴露控制台入口", {
                  description: "请确认 token 已妥善保管",
                });
              }
              setAutoStart.mutate(enabled);
            }}
            aria-label="开机自动开启控制台"
          />
          <span className="text-xs text-muted-foreground">
            开机后自动开启控制台并映射到公网
          </span>
        </div>
      </CardContent>
    </Card>
  );
}

function RunSection({ view }: { view: WebConsoleView }) {
  const start = useStartWebConsole();
  const stop = useStopWebConsole();
  const [confirming, setConfirming] = useState(false);
  const running = view.status.kind === "running";
  const starting = view.status.kind === "starting";

  useEffect(() => {
    if (!confirming) return;
    const timer = setTimeout(() => setConfirming(false), CONFIRM_WINDOW_MS);
    return () => clearTimeout(timer);
  }, [confirming]);

  async function copyUrl() {
    if (!view.publicUrl) return;
    try {
      await writeText(view.publicUrl);
      toast.success("链接已复制（不含 token，可单独发送）");
    } catch {
      toast.error("复制失败，可手动选中链接");
    }
  }

  return (
    <Card>
      <CardContent className="space-y-3 py-4">
        <div className="flex items-center gap-2">
          <span
            className={cn(
              "size-2 shrink-0 rounded-full",
              running
                ? "bg-status-online"
                : starting
                  ? "animate-pulse bg-status-pending"
                  : view.status.kind === "failed"
                    ? "bg-status-error"
                    : "bg-muted-foreground",
            )}
          />
          <h2 className="flex-1 text-sm font-medium">
            {running
              ? "运行中"
              : starting
                ? "启动中"
                : view.status.kind === "failed"
                  ? view.status.message
                  : "未开启"}
          </h2>

          {running || starting ? (
            <Button
              variant="destructive"
              size="sm"
              onClick={() =>
                confirming ? stop.mutate() : setConfirming(true)
              }
              onBlur={() => setConfirming(false)}
              disabled={stop.isPending}
            >
              <PowerOff />
              {confirming ? "确认关闭?" : "关闭"}
            </Button>
          ) : (
            // 没有 token 时禁用；用 span 包裹让 tooltip 仍能触发
            <Hint
              label={
                view.hasToken
                  ? "开启控制台并映射到公网"
                  : "请先生成访问 token"
              }
            >
              <span className="inline-flex">
                <Button
                  size="sm"
                  onClick={() => start.mutate()}
                  disabled={!view.hasToken || start.isPending}
                >
                  {start.isPending ? (
                    <Loader2 className="animate-spin" />
                  ) : (
                    <Power />
                  )}
                  开启
                </Button>
              </span>
            </Hint>
          )}
        </div>

        {view.publicUrl && (
          <div className="flex items-center gap-2">
            <Hint label="点击在浏览器打开">
              <button
                type="button"
                onClick={() => void openUrl(view.publicUrl!)}
                className="selectable min-w-0 flex-1 cursor-pointer truncate text-left font-mono text-xs text-muted-foreground underline-offset-2 outline-none hover:text-primary hover:underline focus-visible:text-primary focus-visible:underline"
              >
                {view.publicUrl}
              </button>
            </Hint>
            <Hint label="复制链接。链接不含 token，可以单独发送">
              <Button variant="outline" size="icon-sm" onClick={() => void copyUrl()}>
                <Copy />
              </Button>
            </Hint>
            <Hint label="在浏览器打开">
              <Button
                variant="outline"
                size="icon-sm"
                onClick={() => void openUrl(view.publicUrl!)}
              >
                <ExternalLink />
              </Button>
            </Hint>
          </div>
        )}

        {running && (
          <p className="text-xs text-muted-foreground">
            在手机浏览器打开上面的链接，输入 token 即可远程查看与开关映射。
          </p>
        )}
      </CardContent>
    </Card>
  );
}
