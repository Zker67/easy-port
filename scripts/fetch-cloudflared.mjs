/**
 * 下载 cloudflared 到 src-tauri/binaries/，供 Tauri 以 sidecar 方式随包分发。
 *
 * 存在原因：二进制约 53 MB，不入库（见 .gitignore）。构建前跑一次即可，
 * 已存在且版本匹配时跳过下载。
 *
 * 用法：
 *   node scripts/fetch-cloudflared.mjs          # 下载默认版本
 *   node scripts/fetch-cloudflared.mjs 2026.9.0 # 指定版本
 *   node scripts/fetch-cloudflared.mjs --local  # 从本机已安装的 cloudflared 复制
 */
import { execFileSync } from "node:child_process";
import { createWriteStream, existsSync, mkdirSync, copyFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { Readable } from "node:stream";
import { pipeline } from "node:stream/promises";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const BIN_DIR = join(ROOT, "src-tauri", "binaries");

/** 与构建机一致的 target triple，Tauri 用它匹配 sidecar 文件名 */
function targetTriple() {
  return execFileSync("rustc", ["--print", "host-tuple"], {
    encoding: "utf8",
  }).trim();
}

/** cloudflared 官方发布产物的平台映射 */
function assetName(triple) {
  if (triple.includes("windows")) {
    return triple.startsWith("aarch64")
      ? "cloudflared-windows-arm64.exe"
      : "cloudflared-windows-amd64.exe";
  }
  if (triple.includes("darwin")) {
    // macOS 官方只发 .tgz，需要额外解包，这里先不自动处理
    throw new Error(
      "macOS 请手动下载并解包 cloudflared-darwin-amd64.tgz 到 src-tauri/binaries/",
    );
  }
  return triple.startsWith("aarch64")
    ? "cloudflared-linux-arm64"
    : "cloudflared-linux-amd64";
}

async function main() {
  const args = process.argv.slice(2);
  const triple = targetTriple();
  const ext = triple.includes("windows") ? ".exe" : "";
  const target = join(BIN_DIR, `cloudflared-${triple}${ext}`);

  mkdirSync(BIN_DIR, { recursive: true });

  if (args.includes("--local")) {
    const which = process.platform === "win32" ? "where" : "which";
    const src = execFileSync(which, ["cloudflared"], { encoding: "utf8" })
      .split(/\r?\n/)[0]
      .trim();
    if (!src) throw new Error("本机未找到 cloudflared");
    copyFileSync(src, target);
    console.log(`已从本机复制：${src} -> ${target}`);
    return;
  }

  if (existsSync(target)) {
    console.log(`已存在，跳过下载：${target}`);
    console.log("需要重新下载时先删除该文件。");
    return;
  }

  const version = args.find((a) => !a.startsWith("--")) ?? "latest";
  const tag = version === "latest" ? "latest/download" : `download/${version}`;
  const url = `https://github.com/cloudflare/cloudflared/releases/${tag}/${assetName(triple)}`;

  console.log(`下载 ${url}`);
  const res = await fetch(url, { redirect: "follow" });
  if (!res.ok) throw new Error(`下载失败：HTTP ${res.status}`);

  await pipeline(Readable.fromWeb(res.body), createWriteStream(target));
  console.log(`已保存：${target}`);
}

main().catch((e) => {
  console.error(String(e.message ?? e));
  process.exit(1);
});
