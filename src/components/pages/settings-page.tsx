import { InfoRow, RowNote } from "@/components/info-row";
import { Card, CardContent } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { useAppVersion } from "@/hooks/use-app-version";
import { useAutostart, useSetAutostart } from "@/hooks/use-autostart";

/**
 * 设置页：只放应用级选项。
 *
 * 引擎信息已拆到独立的「引擎」页，这里不再重复。
 * 排版与引擎页对齐——开关一张卡、信息一张卡，不写大段说明文字。
 */
export function SettingsPage() {
  const autostart = useAutostart();
  const setAutostart = useSetAutostart();
  const version = useAppVersion();

  return (
    <div className="space-y-4">
      <Card>
        <CardContent className="flex items-center justify-between gap-4 py-4">
          <div className="min-w-0 space-y-0.5">
            <Label
              htmlFor="autostart"
              className="cursor-pointer text-sm font-medium"
            >
              开机自启
            </Label>
            <p className="text-xs text-muted-foreground">
              Windows 登录后自动启动 Easy Port
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

      {/* 两个「自动」容易混，这句必须留：它解释的是两个开关的关系，
          不是可有可无的介绍性文字 */}
      <p className="px-1 text-xs text-muted-foreground">
        开机自启只负责拉起应用。要让映射也自动建立，需在对应映射上单独开启
        <span className="text-foreground">「下次启动时自动映射」</span>。
      </p>

      <Card>
        <CardContent className="py-2">
          <dl className="divide-y">
            <InfoRow label="版本">
              <span className="font-mono">{version ? `v${version}` : "—"}</span>
            </InfoRow>
            <InfoRow label="配置">
              <RowNote>
                <span className="font-mono">state.json</span>
                <span className="text-muted-foreground">
                  app data 目录，公网链接不落盘
                </span>
              </RowNote>
            </InfoRow>
            <InfoRow label="引擎">
              <span className="text-muted-foreground">见左侧「引擎」页</span>
            </InfoRow>
          </dl>
        </CardContent>
      </Card>
    </div>
  );
}
