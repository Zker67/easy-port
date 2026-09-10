import { useCallback } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

import { webApi } from "@/lib/tunnel-api";

const KEY = ["web-console"] as const;

export function useWebConsole() {
  return useQuery({
    queryKey: KEY,
    queryFn: webApi.status,
    refetchInterval: 3000,
  });
}

function useInvalidate() {
  const qc = useQueryClient();
  return useCallback(() => {
    void qc.invalidateQueries({ queryKey: KEY });
  }, [qc]);
}

export function useSetWebPort() {
  const invalidate = useInvalidate();
  return useMutation({
    mutationFn: (port: number) => webApi.setPort(port),
    onSuccess: invalidate,
    onError: (err: string) => toast.error(String(err)),
  });
}

export function useSetWebLabel() {
  const invalidate = useInvalidate();
  return useMutation({
    mutationFn: (label: string | null) => webApi.setLabel(label),
    onSuccess: invalidate,
    onError: (err: string) => toast.error(String(err)),
  });
}

/**
 * 生成新 token。
 *
 * 调用方必须把返回的明文展示给用户——**它只在此刻可见一次**，
 * 之后只存哈希，无法再查看。
 */
export function useRegenerateWebToken() {
  const invalidate = useInvalidate();
  return useMutation({
    mutationFn: webApi.regenerateToken,
    onSuccess: () => {
      invalidate();
      toast.success("已生成新 token，旧 token 与已登录设备立即失效");
    },
    onError: (err: string) => toast.error(String(err)),
  });
}

export function useSetWebAutoStart() {
  const invalidate = useInvalidate();
  return useMutation({
    mutationFn: (enabled: boolean) => webApi.setAutoStart(enabled),
    onSuccess: invalidate,
    onError: (err: string) => toast.error(String(err)),
  });
}

export function useStartWebConsole() {
  const invalidate = useInvalidate();
  return useMutation({
    mutationFn: webApi.start,
    onSuccess: () => {
      invalidate();
      toast.success("控制台已开启");
    },
    onError: (err: string) => toast.error(String(err)),
  });
}

export function useStopWebConsole() {
  const invalidate = useInvalidate();
  return useMutation({
    mutationFn: webApi.stop,
    onSuccess: () => {
      invalidate();
      toast.success("控制台已关闭");
    },
    onError: (err: string) => toast.error(String(err)),
  });
}
