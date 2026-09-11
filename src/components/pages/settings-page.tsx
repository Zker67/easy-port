import { Card, CardContent } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { useAutostart, useSetAutostart } from "@/hooks/use-autostart";

export function SettingsPage() {
  const autostart = useAutostart();
  const setAutostart = useSetAutostart();

  return (
    <div className="space-y-6">
      <section className="space-y-3">
        <h2 className="text-sm font-semibold">启动</h2>
        <Card>
          <CardContent className="flex items-center justify-between gap-4 py-4">
            <div className="space-y-1">
              <Label
                htmlFor="autostart"
                className="cursor-pointer text-sm font-medium"
              >
                开机自启
              </Label>
              <p className="text-xs text-muted-foreground">
                Windows 登录后自动启动 Easy Port。
                若还要自动建立映射，需在对应映射上单独开启「下次启动时自动映射」。
              </p>
            </div>
            <Switch
              id="autostart"
              checked={autostart.data ?? false}
              disabled={autostart.isPending || setAutostart.isPending}
              onCheckedChange={(v) => setAutostart.mutate(v)}
              aria-label="开机自启"
            />
          </CardContent>
        </Card>
      </section>

      <section className="space-y-3">
        <h2 className="text-sm font-semibold">关于</h2>
        <Card>
          <CardContent className="space-y-1 py-4 text-xs text-muted-foreground">
            <p>Easy Port — 把本机端口一键映射到公网 HTTPS 链接。</p>
            <p>
              配置保存在系统 app data 目录下的{" "}
              <span className="font-mono">state.json</span>；
              公网链接不会写入任何文件。
            </p>
            <p>穿透引擎的来源与状态见左侧「引擎」页。</p>
          </CardContent>
        </Card>
      </section>
    </div>
  );
}
