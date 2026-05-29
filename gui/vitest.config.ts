import { defineConfig } from 'vitest/config';

// Command-core tests run in a Node environment (no DOM needed). The core is
// framework-agnostic, so this config is intentionally minimal.
export default defineConfig({
  test: {
    environment: 'node',
    include: ['src/core/**/*.test.ts'],
  },
});
