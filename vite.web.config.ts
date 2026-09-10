import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "node:path";

/**
 * Web 远程控制台的构建配置，与桌面端完全分开。
 *
 * 为什么不复用桌面端的 src/：那会把整套桌面依赖拖进手机端——
 * `@tauri-apps/api` 在浏览器里根本不可用，zustand / TanStack Query /
 * shadcn 全家桶对两个视图来说也过重。手机端首要指标是加载速度。
 *
 * 两端**只共用类型**（src/lib/tunnel-types.ts），逻辑不共用。
 *
 * 产物进 dist-web/，由 Rust 侧 include_dir! 编进二进制。
 */
export default defineConfig({
  root: path.resolve(import.meta.dirname, "./src-web"),
  plugins: [react()],
  resolve: {
    alias: {
      "@shared": path.resolve(import.meta.dirname, "./src/lib"),
      // 用 preact 替换 react：同样的代码，产物从 70 KB 降到 ~13 KB（gzip）。
      // 桌面端不受影响，那边仍是完整的 react。
      react: "preact/compat",
      "react-dom": "preact/compat",
      "react/jsx-runtime": "preact/jsx-runtime",
    },
  },
  build: {
    outDir: path.resolve(import.meta.dirname, "./dist-web"),
    emptyOutDir: true,
    // 手机端，单文件更省一次往返
    assetsInlineLimit: 4096,
  },
});
