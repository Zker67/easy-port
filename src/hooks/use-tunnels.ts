import { useCallback, useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

import { tunnelApi } from "@/lib/tunnel-api";

const KEYS = {
  engine: ["engine"] as const,
  tunnels: ["tunnels"] as const,
  counts: ["counts"] as const,
  autoStart: ["auto-start"] as const,
  allTags: ["all-tags"] as const,
};

/** cloudflared 可用性只需查一次，除非用户手动重试 */
export function useEngineStatus() {
  return useQuery({
    queryKey: KEYS.engine,
    queryFn: tunnelApi.checkEngine,
    staleTime: Infinity,
    retry: false,
  });
}

export function useTunnels() {
  return useQuery({
    queryKey: KEYS.tunnels,
    queryFn: tunnelApi.list,
    refetchInterval: 3000,
  });
}

export function useTunnelCounts() {
  return useQuery({
    queryKey: KEYS.counts,
    queryFn: tunnelApi.counts,
    refetchInterval: 3000,
  });
}

export function useAutoStartFlags() {
  return useQuery({
    queryKey: KEYS.autoStart,
    queryFn: tunnelApi.autoStartFlags,
    refetchInterval: 3000,
  });
}

/** 建立隧道的最长等待秒数，来自 Rust 侧常量 */
export function useUrlTimeoutSecs() {
  const query = useQuery({
    queryKey: ["url-timeout"] as const,
    queryFn: tunnelApi.urlTimeoutSecs,
    staleTime: Infinity,
  });
  return query.data ?? 30;
}

/**
 * 建立隧道期间的递进等待文案。
 *
 * 存在原因：cloudflared 最长可能等 30 秒，期间界面上只有一个转圈，
 * 用户的合理判断是「卡死了」而不是「在等」。这里纯前端计时，
 * 不改建立流程，也不需要 Rust 侧回报进度。
 */
export function useWaitingHint(active: boolean, timeoutSecs: number): string | null {
  const [elapsed, setElapsed] = useState(0);

  useEffect(() => {
    if (!active) {
      setElapsed(0);
      return;
    }
    const started = Date.now();
    const timer = setInterval(() => {
      setElapsed(Math.floor((Date.now() - started) / 1000));
    }, 500);
    return () => clearInterval(timer);
  }, [active]);

  if (!active) return null;
  if (elapsed < 3) return "正在启动 cloudflared…";
  if (elapsed < 10) return "正在向 Cloudflare 申请公网域名…";
  return `仍在等待，网络较慢时最长需要 ${timeoutSecs} 秒（已等待 ${elapsed} 秒）`;
}

/** 引用保持稳定，供 effect 依赖使用 */
function useInvalidate() {
  const qc = useQueryClient();
  return useCallback(() => {
    void qc.invalidateQueries({ queryKey: KEYS.tunnels });
    void qc.invalidateQueries({ queryKey: KEYS.counts });
    void qc.invalidateQueries({ queryKey: KEYS.autoStart });
    // 标签列表随条目变化：改标签、删条目都可能让某个标签凭空出现或消失
    void qc.invalidateQueries({ queryKey: KEYS.allTags });
  }, [qc]);
}

export function useCreateTunnel() {
  const invalidate = useInvalidate();
  return useMutation({
    mutationFn: ({
      port,
      label,
      expireMinutes,
    }: {
      port: number;
      label?: string;
      expireMinutes?: number;
    }) => tunnelApi.create(port, label, expireMinutes),
    onSuccess: (tunnel) => {
      invalidate();
      toast.success(`端口 ${tunnel.port} 已映射`, {
        description: tunnel.publicUrl ?? undefined,
      });
    },
    onError: (err: string) => toast.error(String(err)),
  });
}

export function useSetExpiry() {
  const invalidate = useInvalidate();
  return useMutation({
    mutationFn: ({ id, minutes }: { id: string; minutes: number | null }) =>
      tunnelApi.setExpiry(id, minutes),
    onSuccess: (_deadline, { minutes }) => {
      invalidate();
      toast.success(
        minutes === null
          ? "已取消定时关闭"
          : `将在 ${formatDuration(minutes)}后自动断开`,
      );
    },
    onError: (err: string) => toast.error(String(err)),
  });
}

/** 把分钟数写成中文时长，例如 90 → 1 小时 30 分钟 */
export function formatDuration(minutes: number): string {
  if (minutes < 60) return `${minutes} 分钟`;
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  return m === 0 ? `${h} 小时` : `${h} 小时 ${m} 分钟`;
}

/**
 * 到期倒计时文本，每秒刷新；`expiresAt` 为空时返回 null。
 *
 * 只做展示，真正的断开由 Rust 侧定时任务负责——
 * 前端计时不可靠（窗口最小化、系统休眠都会影响），不能拿它当执行依据。
 */
export function useCountdown(expiresAt: string | null): string | null {
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    if (!expiresAt) return;
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, [expiresAt]);

  if (!expiresAt) return null;

  const remain = Math.max(0, new Date(expiresAt).getTime() - now);
  const total = Math.floor(remain / 1000);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;

  if (total <= 0) return "即将断开";
  if (h > 0) return `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
  return `${m}:${String(s).padStart(2, "0")}`;
}

/** 带上 port 只为让提示语能分辨是哪一条，command 签名未变 */
export function useStopTunnel() {
  const invalidate = useInvalidate();
  return useMutation({
    mutationFn: ({ id }: { id: string; port: number }) => tunnelApi.stop(id),
    onSuccess: (_data, { port }) => {
      invalidate();
      toast.success(`端口 ${port} 的映射已断开`);
    },
    onError: (err: string) => toast.error(String(err)),
  });
}

export function useRemoveTunnel() {
  const invalidate = useInvalidate();
  return useMutation({
    mutationFn: ({ id }: { id: string; port: number }) => tunnelApi.remove(id),
    onSuccess: (_data, { port }) => {
      invalidate();
      toast.success(`端口 ${port} 的记录已移除`);
    },
    onError: (err: string) => toast.error(String(err)),
  });
}

/** 修改备注。成功提示刻意从简：这是高频轻量操作，不该每次都弹一条 toast */
export function useSetLabel() {
  const invalidate = useInvalidate();
  return useMutation({
    mutationFn: ({ id, label }: { id: string; label: string | null }) =>
      tunnelApi.setLabel(id, label),
    onSuccess: invalidate,
    onError: (err: string) => toast.error(String(err)),
  });
}

/** 当前用到的全部标签。随条目变化，跟着列表一起轮询 */
export function useAllTags() {
  return useQuery({
    queryKey: KEYS.allTags,
    queryFn: tunnelApi.allTags,
    refetchInterval: 3000,
  });
}

export function useSetTags() {
  const invalidate = useInvalidate();
  return useMutation({
    mutationFn: ({ id, tags }: { id: string; tags: string[] }) =>
      tunnelApi.setTags(id, tags),
    onSuccess: invalidate,
    onError: (err: string) => toast.error(String(err)),
  });
}

export function useSetFavorite() {
  const invalidate = useInvalidate();
  return useMutation({
    mutationFn: ({ id, favorite }: { id: string; favorite: boolean }) =>
      tunnelApi.setFavorite(id, favorite),
    onSuccess: invalidate,
    onError: (err: string) => toast.error(String(err)),
  });
}

/** 归档单条：从映射页移走，记录保留在历史页 */
export function useSetArchived() {
  const invalidate = useInvalidate();
  return useMutation({
    mutationFn: ({ id, archived }: { id: string; archived: boolean }) =>
      tunnelApi.setArchived(id, archived),
    onSuccess: (_data, { archived }) => {
      invalidate();
      toast.success(archived ? "已归档，可在历史中找到" : "已恢复到映射列表");
    },
    onError: (err: string) => toast.error(String(err)),
  });
}

/** 一键归档映射页里所有已断开的条目 */
export function useArchiveInactive() {
  const invalidate = useInvalidate();
  return useMutation({
    mutationFn: tunnelApi.archiveInactive,
    onSuccess: (count) => {
      invalidate();
      toast.success(
        count > 0 ? `已归档 ${count} 条已断开的映射` : "没有可归档的映射",
      );
    },
    onError: (err: string) => toast.error(String(err)),
  });
}

/** 彻底删除所有已归档记录（历史页的「清空」） */
export function usePurgeArchived() {
  const invalidate = useInvalidate();
  return useMutation({
    mutationFn: tunnelApi.purgeArchived,
    onSuccess: (count) => {
      invalidate();
      toast.success(count > 0 ? `已删除 ${count} 条记录` : "没有可删除的记录");
    },
    onError: (err: string) => toast.error(String(err)),
  });
}

export function useSetAutoStart() {
  const invalidate = useInvalidate();
  return useMutation({
    mutationFn: ({ id, enabled }: { id: string; enabled: boolean }) =>
      tunnelApi.setAutoStart(id, enabled),
    onSuccess: invalidate,
    onError: (err: string) => toast.error(String(err)),
  });
}

/**
 * 启动时重建标记了自动重连的隧道，整个应用生命周期内只跑一次。
 *
 * 必须在确认引擎可用后调用；引擎不可用时重建必然逐条失败。
 */
export function useRestoreOnLaunch(enabled: boolean) {
  const invalidate = useInvalidate();
  const done = useRef(false);

  useEffect(() => {
    if (!enabled || done.current) return;
    done.current = true;

    void (async () => {
      // 启动时若读配置有告警（文件损坏等），先提示一次。
      const warning = await tunnelApi.startupWarning();
      if (warning) toast.warning(warning);

      const outcomes = await tunnelApi.restore();
      if (outcomes.length === 0) return;

      invalidate();
      const ok = outcomes.filter((o) => o.ok).length;
      if (ok > 0) toast.success(`已自动重连 ${ok} 条映射（链接为新分配）`);
      for (const failed of outcomes.filter((o) => !o.ok)) {
        toast.error(`端口 ${failed.port} 自动重连失败：${failed.error}`);
      }
    })();
  }, [enabled, invalidate]);
}
