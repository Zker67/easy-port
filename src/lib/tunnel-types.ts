/**
 * 桌面端与 Web 控制台共用的类型定义。
 *
 * 单独成文件是为了让两端从同一处导入，字段不会漂移。
 * **这里只放类型，不放任何依赖 Tauri IPC 的代码**——
 * Web 端跑在普通浏览器里，`@tauri-apps/api` 在那里不可用。
 */

/** 与 Rust 侧 tunnel::provider::TunnelStatus 对应 */
export type TunnelStatus =
  | { kind: "starting" }
  | { kind: "running" }
  | { kind: "stopped" }
  | { kind: "failed"; message: string };

/** 与 Rust 侧 tunnel::provider::SiteInfo 对应 */
export interface SiteInfo {
  title: string | null;
  /** favicon 的 data URI */
  icon: string | null;
}

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

/**
 * Web 端可见的映射视图，与 Rust 侧 web::server::WebTunnelView 对应。
 *
 * 刻意比 `Tunnel` 少：不含 favicon（可能上百 KB）、不含归档收藏等
 * 桌面端才用得上的字段。收敛发生在 Rust 侧，这里只是把它写清楚。
 */
export interface WebTunnelView {
  id: string;
  port: number;
  label: string | null;
  publicUrl: string | null;
  status: TunnelStatus;
  siteTitle: string | null;
  tags: string[];
}

/** 运行中（含启动中）视为活跃 */
export function isActive(status: TunnelStatus): boolean {
  return status.kind === "running" || status.kind === "starting";
}
