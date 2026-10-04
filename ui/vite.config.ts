import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { existsSync } from 'node:fs';

// Web版とデスクトップ版は同じソースから --mode web / --mode desktop で出し分ける。
export default defineConfig(({ mode }) => {
  if (mode === 'web' && !existsSync(new URL('./src/lib/engine/pkg/formdoc_wasm.js', import.meta.url))) {
    throw new Error('Web版のエンジン（ui/src/lib/engine/pkg）がありません。先にリポジトリのルートで `bun ready` を実行してください。');
  }
  return {
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
  };
});
