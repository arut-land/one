import { defineConfig } from "vite";

export default defineConfig({
  build: {
    target: "node20",
    assetsInlineLimit: Number.POSITIVE_INFINITY,
    lib: {
      entry: "src/extension.ts",
      formats: ["es"],
    },
    rollupOptions: {
      external: ["vscode", "node:crypto"],
      output: {
        entryFileNames: "extension.js",
        format: "es",
      },
    },
  },
});
