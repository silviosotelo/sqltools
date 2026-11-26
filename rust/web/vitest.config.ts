import { defineConfig } from "vitest/config";

// Pure-logic tests only (tree model, environment validation, token contrast): no DOM needed.
export default defineConfig({
  test: {
    environment: "node",
    include: ["src/**/*.test.ts"],
  },
});
