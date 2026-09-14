import react from "@vitejs/plugin-react";
import type { UserConfig } from "vite";

/** The base every browser root shares: React with the compiler, nothing else. */
export function browserApp(config: UserConfig = {}): UserConfig {
  return { plugins: [react({ compiler: true })], ...config };
}
