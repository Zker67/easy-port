import { invoke } from "@tauri-apps/api/core";

// 类型定义在 tunnel-types.ts，与 Web 控制台共用一份，避免字段漂移。
// 这里原样转出，调用方 import 路径不变。
export type { SiteInfo, Tunnel, TunnelStatus } from "./tunnel-types";
export { isActive } from "./tunnel-types";

import type { Tunnel } from "./tunnel-types";

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
  /** true = 用的是内嵌释放出的副本；false = 回退到系统 PATH（不变量 4）*/
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

/** 与 Rust 侧 web::console::WebStatus 对应 */
export type WebStatus =
  | { kind: "stopped" }
  | { kind: "starting" }
  | { kind: "running" }
  | { kind: "failed"; message: string };

/**
 * 与 Rust 侧 web::console::WebConsoleView 对应。
 *
 * 刻意不含 token 哈希——前端没有任何理由拿到它。
 */
export interface WebConsoleView {
  port: number;
  label: string | null;
  /** 是否已设置 token；开启控制台前必须为 true */
  hasToken: boolean;
  status: WebStatus;
  /** 公网链接，仅运行时非空。不落盘 */
  publicUrl: string | null;
  autoStart: boolean;
}

/**
 * Web 远程控制台。
 *
 * 这些命令**只在桌面端可用**：Web 端不能改控制台自身的配置，
 * 否则攻破一次即可把 token 改成攻击者的，永久驻留。
 */
export const webApi = {
  status: () => invoke<WebConsoleView>("web_console_status"),

  setPort: (port: number) => invoke<void>("set_web_console_port", { port }),

  setLabel: (label: string | null) =>
    invoke<void>("set_web_console_label", { label }),

  /** 生成新 token；**明文只在此刻返回一次**，之后只能重新生成 */
  regenerateToken: () => invoke<string>("regenerate_web_token"),

  setAutoStart: (enabled: boolean) =>
    invoke<void>("set_web_auto_start", { enabled }),

  start: () => invoke<WebConsoleView>("start_web_console"),

  stop: () => invoke<void>("stop_web_console"),

  /** 启动时按 auto_start 决定是否自动开启，返回是否真的开启了 */
  restore: () => invoke<boolean>("restore_web_console"),
};
