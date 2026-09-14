import { browserApp } from "@arut/chat-ui/vite";
import { defineConfig } from "vite";

// Two bundles from one root: the extension host module that owns the session,
// and the webview page that renders it (`vite build --mode webview`).
export default defineConfig(({ mode }) =>
  mode === "webview"
    ? browserApp({
        // A library build leaves `process.env` to its consumer; a webview has
        // no `process`, so React's build switch is resolved here.
        define: { "process.env.NODE_ENV": JSON.stringify("production") },
        build: {
          emptyOutDir: false,
          cssCodeSplit: false,
          lib: { entry: "src/webview.tsx", formats: ["iife"], name: "arutChat", fileName: () => "webview.js" },
          rollupOptions: { output: { assetFileNames: "webview.css" } },
        },
      })
    : {
        build: {
          target: "node20",
          assetsInlineLimit: Number.POSITIVE_INFINITY,
          lib: { entry: "src/extension.ts", formats: ["es"] },
          rollupOptions: {
            external: ["vscode", "node:crypto"],
            output: { entryFileNames: "extension.js", format: "es" },
          },
        },
      },
);
