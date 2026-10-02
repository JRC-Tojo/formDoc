import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// Web版とデスクトップ版は同じソースから --mode web / --mode desktop で出し分ける。
export default defineConfig(({ mode }) => ({
  plugins: [svelte()],
  clearScreen: false,
  base: './',
  server: { port: 5173, strictPort: true },
  build: {
    outDir: mode === 'desktop' ? 'dist-desktop' : 'dist-web',
    target: 'es2022',
    emptyOutDir: true,
  },
  worker: { format: 'es' },
  envPrefix: ['VITE_', 'TAURI_ENV_'],
}));
