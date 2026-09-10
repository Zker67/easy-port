import { invoke } from "@tauri-apps/api/core";

/** 与 Rust 侧 tunnel::provider::TunnelStatus 对应 */
export type TunnelStatus =
  | { kind: "starting" }
  | { kind: "running" }
  | { kind: "stopped" }
  | { kind: "failed"; message: string };

/** 与 Rust 侧 tunnel::provider::Tunnel 对应 */
export interface Tunnel {
  id: string;
  port: number;
  label: string | null;
  publicUrl: string | null;
  status: TunnelStatus;
  createdAt: string;
}

/** 与 Rust 侧 tunnel::registry::TunnelCounts 对应 */
export interface TunnelCounts {
  active: number;
  totalCreated: number;
}

/** 与 Rust 侧 tunnel::provider::EngineStatus 对应 */
export interface EngineStatus {
  available: boolean;
  version: string | null;
  engine: string;
}

export const tunnelApi = {
  checkEngine: () => invoke<EngineStatus>("check_engine"),

  create: (port: number, label?: string) =>
    invoke<Tunnel>("create_tunnel", { port, label: label || null }),

  stop: (id: string) => invoke<void>("stop_tunnel", { id }),

  remove: (id: string) => invoke<void>("remove_tunnel", { id }),

  list: () => invoke<Tunnel[]>("list_tunnels"),

  counts: () => invoke<TunnelCounts>("tunnel_counts"),
};

export function isActive(status: TunnelStatus): boolean {
  return status.kind === "running" || status.kind === "starting";
}
