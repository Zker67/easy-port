import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";

/**
 * 应用版本号，取自 `tauri.conf.json`。
 *
 * 不在前端硬编码：版本号已经在 `tauri.conf.json` / `Cargo.toml` / `package.json`
 * 三处维护，再抄一份到 TSX 里必然漂移。读不到时返回 null，
 * 由调用方决定不显示——版本号不是关键信息，缺了不该让界面出错。
 */
export function useAppVersion(): string | null {
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    void getVersion()
      .then((v) => {
        if (alive) setVersion(v);
      })
      .catch(() => {
        // 取不到就不显示，不弹错误：这只是个装饰性信息
      });
    return () => {
      alive = false;
    };
  }, []);

  return version;
}
