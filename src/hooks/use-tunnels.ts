import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

import { tunnelApi } from "@/lib/tunnel-api";

const KEYS = {
  engine: ["engine"] as const,
  tunnels: ["tunnels"] as const,
  counts: ["counts"] as const,
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

function useInvalidate() {
  const qc = useQueryClient();
  return () => {
    void qc.invalidateQueries({ queryKey: KEYS.tunnels });
    void qc.invalidateQueries({ queryKey: KEYS.counts });
  };
}

export function useCreateTunnel() {
  const invalidate = useInvalidate();
  return useMutation({
    mutationFn: ({ port, label }: { port: number; label?: string }) =>
      tunnelApi.create(port, label),
    onSuccess: (tunnel) => {
      invalidate();
      toast.success(`端口 ${tunnel.port} 已映射`, {
        description: tunnel.publicUrl ?? undefined,
      });
    },
    onError: (err: string) => toast.error(String(err)),
  });
}

export function useStopTunnel() {
  const invalidate = useInvalidate();
  return useMutation({
    mutationFn: (id: string) => tunnelApi.stop(id),
    onSuccess: () => {
      invalidate();
      toast.success("映射已断开");
    },
    onError: (err: string) => toast.error(String(err)),
  });
}

export function useRemoveTunnel() {
  const invalidate = useInvalidate();
  return useMutation({
    mutationFn: (id: string) => tunnelApi.remove(id),
    onSuccess: invalidate,
    onError: (err: string) => toast.error(String(err)),
  });
}
