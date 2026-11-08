import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    environment: "node",
    include: ["test/**/*.test.ts"],
    testTimeout: 120_000,
    // A forced GC (`globalThis.gc`) for the allocation and retained-heap checks of the render
    // loop, so they measure the frame rather than where the collector happens to be.
    pool: "forks",
    poolOptions: { forks: { execArgv: ["--expose-gc"] } },
  },
  resolve: {
    alias: { "@vwp/protocol": new URL("../protocol/src/index.ts", import.meta.url).pathname },
  },
});
