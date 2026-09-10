/**
 * 从 shadcn 官方 registry 拉取组件源码到 src/components/ui。
 *
 * 存在原因：shadcn CLI 的 `add` 在安装依赖时会执行畸形命令 `npm install -- cn`
 * （`cn` 并非 npm 包，而是本项目的 @/lib/utils 工具函数），在 npm 11 下必然失败。
 * 这里改用 CLI 的只读命令 `view` 取同一份官方源码，自己落盘并重写 import。
 *
 * 用法：node scripts/fetch-shadcn.mjs button input card badge
 */
import { execFileSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

const components = process.argv.slice(2);
if (components.length === 0) {
  console.error("用法: node scripts/fetch-shadcn.mjs <component>...");
  process.exit(1);
}

const UI_DIR = join(process.cwd(), "src", "components", "ui");

/** registry 把 cn 当独立包引用，本项目中它来自 @/lib/utils */
function rewriteImports(content) {
  return content.replace(/from ["']cn["']/g, 'from "@/lib/utils"');
}

for (const name of components) {
  process.stdout.write(`拉取 ${name} ... `);
  const raw = execFileSync(
    "npx",
    ["--yes", "shadcn@latest", "view", name],
    { encoding: "utf8", maxBuffer: 32 * 1024 * 1024, shell: true },
  );

  const start = raw.indexOf("[");
  if (start === -1) throw new Error(`${name}: registry 未返回 JSON`);
  const items = JSON.parse(raw.slice(start));

  for (const item of items) {
    for (const file of item.files ?? []) {
      const target = join(UI_DIR, file.path.split("/").pop());
      mkdirSync(dirname(target), { recursive: true });
      writeFileSync(target, rewriteImports(file.content), "utf8");
      process.stdout.write(`→ ${target.replace(process.cwd() + "\\", "")} `);
    }
  }
  console.log("✓");
}
