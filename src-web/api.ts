import type { WebTunnelView } from "@shared/tunnel-types";

/**
 * Web 端与内嵌服务之间是普通 HTTP JSON，不是 Tauri IPC。
 *
 * 会话走 HttpOnly cookie，因此这里不碰任何 token——
 * 前端读不到也不该读（读得到就意味着 XSS 能偷走）。
 */

/** 会话失效时抛出，供上层切回登录视图 */
export class Unauthorized extends Error {
  constructor() {
    super("请重新登录");
  }
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(path, {
    // 同源请求，cookie 会自动带上；显式写出以免将来改动踩坑
    credentials: "same-origin",
    ...init,
  });

  if (res.status === 401) throw new Unauthorized();
  if (!res.ok) {
    const msg = await res
      .json()
      .then((b: { error?: string }) => b.error)
      .catch(() => null);
    throw new Error(msg ?? `请求失败（${res.status}）`);
  }

  // 部分接口只回状态码，没有 body
  const text = await res.text();
  return (text ? JSON.parse(text) : null) as T;
}

export const api = {
  login: (token: string) =>
    request<{ ok: boolean }>("/api/login", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ token }),
    }),

  logout: () => request<null>("/api/logout", { method: "POST" }),

  list: () => request<WebTunnelView[]>("/api/tunnels"),

  start: (id: string) =>
    request<null>(`/api/tunnels/${encodeURIComponent(id)}/start`, {
      method: "POST",
    }),

  stop: (id: string) =>
    request<null>(`/api/tunnels/${encodeURIComponent(id)}/stop`, {
      method: "POST",
    }),
};
