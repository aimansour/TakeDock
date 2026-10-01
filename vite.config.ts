import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { svelteTesting } from '@testing-library/svelte/vite';
export default defineConfig({
  plugins: [svelte(), svelteTesting({ autoCleanup: false })],
  resolve: { preserveSymlinks: true },
  clearScreen: false,
  test: {
    environment: 'jsdom',
    setupFiles: ['./src/test-setup.ts'],
    include: ['src/**/*.test.ts'],
  },
  server: { host: '127.0.0.1', port: 1420, strictPort: true },
  build: { target: 'es2022', sourcemap: false },
});
