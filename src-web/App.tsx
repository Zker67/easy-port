import { useCallback, useEffect, useState } from "react";

import { isActive, type WebTunnelView } from "@shared/tunnel-types";
import { api, Unauthorized } from "./api";

/**
 * Web 控制台。两个视图：登录、列表。
 *
 * 刻意不引路由库——两个视图用一个布尔量就够，为它加依赖不值当。
 */
export default function App() {
  // 初始按「已登录」试一次：cookie 还在的话可以跳过登录页
  const [authed, setAuthed] = useState<boolean | null>(null);

  if (authed === false) return <Login onDone={() => setAuthed(true)} />;
  return <List authed={authed} onAuthChange={setAuthed} />;
}

function Login({ onDone }: { onDone: () => void }) {
  const [token, setToken] = useState("");
  const [show, setShow] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    if (!token.trim() || busy) return;
    setBusy(true);
    setError(null);
    try {
      await api.login(token.trim());
      onDone();
    } catch {
      // 固定文案，不区分「token 错误」与「已被限速」：
      // 区分了等于告诉攻击者「换个 IP 继续」
      setError("认证失败，请检查 token");
      setToken("");
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="wrap">
      <form className="login" onSubmit={submit}>
        <h1>Easy Port 控制台</h1>
        <p>请输入访问 token。token 在桌面端的「Web」标签页生成。</p>
        <div className="field">
          <input
            // 43 位随机串手输极易出错，允许切换明文核对
            type={show ? "text" : "password"}
            value={token}
            onChange={(e) => setToken(e.target.value)}
            placeholder="访问 token"
            aria-label="访问 token"
            autoComplete="current-password"
            autoCapitalize="none"
            autoCorrect="off"
            spellCheck={false}
          />
          <button
            type="button"
            onClick={() => setShow((s) => !s)}
            aria-label={show ? "隐藏 token" : "显示 token"}
          >
            {show ? "隐藏" : "显示"}
          </button>
        </div>
        {error && <p className="error">{error}</p>}
        <button className="primary" type="submit" disabled={busy || !token.trim()}>
          {busy ? "验证中…" : "进入"}
        </button>
      </form>
    </div>
  );
}

function List({
  authed,
  onAuthChange,
}: {
  authed: boolean | null;
  onAuthChange: (v: boolean) => void;
}) {
  const [items, setItems] = useState<WebTunnelView[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState<string | null>(null);
  const [confirming, setConfirming] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      setItems(await api.list());
      onAuthChange(true);
      setError(null);
    } catch (e) {
      if (e instanceof Unauthorized) {
        onAuthChange(false);
        return;
      }
      setError(e instanceof Error ? e.message : "加载失败");
    }
  }, [onAuthChange]);

  useEffect(() => {
    void load();
    // 与桌面端一致的轮询节奏
    const timer = setInterval(() => void load(), 3000);
    return () => clearInterval(timer);
  }, [load]);

  async function act(id: string, fn: () => Promise<unknown>) {
    setPending(id);
    setError(null);
    try {
      await fn();
      await load();
    } catch (e) {
      if (e instanceof Unauthorized) onAuthChange(false);
      else setError(e instanceof Error ? e.message : "操作失败");
    } finally {
      setPending(null);
      setConfirming(null);
    }
  }

  async function copy(url: string) {
    try {
      await navigator.clipboard.writeText(url);
      setError(null);
    } catch {
      // 不谎报成功。手机浏览器在非安全上下文下会拒绝剪贴板 API
      setError("复制失败，请长按链接手动复制");
    }
  }

  if (authed === null && items === null) {
    return (
      <div className="wrap">
        <p className="note">加载中…</p>
      </div>
    );
  }

  return (
    <div className="wrap">
      <div className="bar">
        <h1>映射</h1>
        <button
          onClick={() =>
            void api.logout().finally(() => onAuthChange(false))
          }
        >
          退出登录
        </button>
      </div>

      {error && <p className="error">{error}</p>}

      {items && items.length === 0 ? (
        <div className="empty">
          还没有映射。
          <br />
          新建映射请在桌面端操作。
        </div>
      ) : (
        <div className="list">
          {items?.map((t) => {
            const on = isActive(t.status);
            const busy = pending === t.id;
            return (
              <div className="item" key={t.id}>
                <div className="row">
                  <span className={on ? "dot on" : "dot"} />
                  <span className={on ? "port on" : "port"}>{t.port}</span>
                  <span className="title">
                    {t.siteTitle ?? t.label ?? ""}
                  </span>
                </div>

                {t.publicUrl && (
                  <a
                    className="url"
                    href={t.publicUrl}
                    target="_blank"
                    rel="noreferrer noopener"
                  >
                    {t.publicUrl}
                  </a>
                )}

                {t.tags.length > 0 && (
                  <div className="tags">
                    {t.tags.map((tag) => (
                      <span className="tag" key={tag}>
                        {tag}
                      </span>
                    ))}
                  </div>
                )}

                <div className="actions">
                  {on ? (
                    <>
                      {t.publicUrl && (
                        <button onClick={() => void copy(t.publicUrl!)}>
                          复制链接
                        </button>
                      )}
                      {/* 断开会让链接立即失效且无法恢复为同一条，需二次确认 */}
                      <button
                        className="danger"
                        disabled={busy}
                        onClick={() =>
                          confirming === t.id
                            ? void act(t.id, () => api.stop(t.id))
                            : setConfirming(t.id)
                        }
                        onBlur={() => setConfirming(null)}
                      >
                        {confirming === t.id ? "确认断开?" : "断开"}
                      </button>
                    </>
                  ) : (
                    <button
                      className="primary"
                      disabled={busy}
                      onClick={() => void act(t.id, () => api.start(t.id))}
                    >
                      {busy ? "连接中…" : "开启"}
                    </button>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      )}

      <p className="note">
        开启后会分配一条新链接，旧链接无法恢复。
      </p>
    </div>
  );
}
