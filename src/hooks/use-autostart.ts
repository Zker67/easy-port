import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { toast } from "sonner";

const KEY = ["autostart"] as const;

/**
 * 系统开机自启（Windows 走注册表 Run 项，由官方 autostart 插件管理）。
 *
 * 注意与隧道的「下次启动时自动映射」是两件事：
 * 这里控制的是「Windows 开机后是否拉起 Easy Port」，
 * 那个控制的是「Easy Port 启动后是否自动建立某条映射」。两者叠加才实现开机即映射。
 */
export function useAutostart() {
  return useQuery({
    queryKey: KEY,
    queryFn: isEnabled,
    staleTime: Infinity,
    retry: false,
  });
}

export function useSetAutostart() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (next: boolean) => {
      if (next) await enable();
      else await disable();
      return next;
    },
    onSuccess: (next) => {
      void qc.invalidateQueries({ queryKey: KEY });
      toast.success(next ? "已开启开机自启" : "已关闭开机自启");
    },
    onError: (err) => toast.error(`设置开机自启失败：${String(err)}`),
  });
}
