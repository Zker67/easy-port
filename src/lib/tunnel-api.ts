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
  /** 备注：每个端口一条的自由文本，说明这个端口是干什么的 */
  label: string | null;
  publicUrl: string | null;
  status: TunnelStatus;
  createdAt: string;
  /** 定时关闭的到期时刻（RFC 3339）；未设定为 null。纯运行态，不跨重启 */
  expiresAt: string | null;
  /** 已归档：从「映射」页隐藏，但仍保留在「历史」页 */
  archived: boolean;
  /** 已收藏：在「映射」页置顶成独立分区 */
  favorite: boolean;
  /** 本机服务的站点信息，异步探测所得；非网页或探测失败为 null */
  site: SiteInfo | null;
  /** 人工分类标签，可多个、跨端口复用，用于筛选。与 label（备注）是两种东西 */
  tags: string[];
}

/** 与 Rust 侧 tunnel::provider::SiteInfo 对应 */
export interface SiteInfo {
  title: string | null;
  /** favicon 的 data URI */
  icon: string | null;
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
  /** 可执行文件路径，未找到时为 null */
  path: string | null;
  /** 是否随应用打包。见 AGENTS.md 不变量 4：恒为 false */
  bundled: boolean;
}

/** 与 Rust 侧 commands::RestoreOutcome 对应 */
export interface RestoreOutcome {
  port: number;
  ok: boolean;
  error: string | null;
}

export const tunnelApi = {
  checkEngine: () => invoke<EngineStatus>("check_engine"),

  startupWarning: () => invoke<string | null>("startup_warning"),

  /** 建立隧道的最长等待秒数，等待提示引用它以免与 Rust 侧硬编码漂移 */
  urlTimeoutSecs: () => invoke<number>("url_timeout_secs"),

  create: (port: number, label?: string, expireMinutes?: number) =>
    invoke<Tunnel>("create_tunnel", {
      port,
      label: label || null,
      expireMinutes: expireMinutes ?? null,
    }),

  /** 设定/取消定时关闭；minutes 为 null 表示取消。返回到期时刻 */
  setExpiry: (id: string, minutes: number | null) =>
    invoke<string | null>("set_expiry", { id, minutes }),

  stop: (id: string) => invoke<void>("stop_tunnel", { id }),

  remove: (id: string) => invoke<void>("remove_tunnel", { id }),

  setAutoStart: (id: string, enabled: boolean) =>
    invoke<void>("set_auto_start", { id, enabled }),

  /** 重建标记了自动重连的隧道；拿到的是**新链接**，不是恢复旧链接 */
  restore: () => invoke<RestoreOutcome[]>("restore_tunnels"),

  list: () => invoke<Tunnel[]>("list_tunnels"),

  autoStartFlags: () => invoke<Record<string, boolean>>("auto_start_flags"),

  /** 修改备注；传 null 或空串即清除。与运行状态无关，随时可改 */
  setLabel: (id: string, label: string | null) =>
    invoke<void>("set_label", { id, label }),

  /** 覆盖式设置标签；去重与裁空白由 Rust 侧完成 */
  setTags: (id: string, tags: string[]) =>
    invoke<void>("set_tags", { id, tags }),

  /** 当前用到的全部标签，供筛选栏与输入建议使用 */
  allTags: () => invoke<string[]>("all_tags"),

  /** 收藏/取消收藏 */
  setFavorite: (id: string, favorite: boolean) =>
    invoke<void>("set_favorite", { id, favorite }),

  /** 归档/取消归档单条；归档只是从映射页隐藏，记录仍在历史页 */
  setArchived: (id: string, archived: boolean) =>
    invoke<void>("set_archived", { id, archived }),

  /** 归档所有已断开/失败的记录，返回归档条数 */
  archiveInactive: () => invoke<number>("archive_inactive"),

  /** 彻底删除所有已归档的记录，返回删除条数 */
  purgeArchived: () => invoke<number>("purge_archived"),

  counts: () => invoke<TunnelCounts>("tunnel_counts"),
};

export function isActive(status: TunnelStatus): boolean {
  return status.kind === "running" || status.kind === "starting";
}
