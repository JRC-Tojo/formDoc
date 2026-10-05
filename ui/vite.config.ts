import { defineConfig, type Plugin } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { existsSync, readFileSync } from 'node:fs';

// アプリの版（ui/package.json。scripts/bump-version.ts が Cargo.toml と一緒に書き換える）と、ビルドの識別子。
// Web版は配信中の version.json の build と比べて、新しい版が公開されたことを知る。
const version: string = JSON.parse(readFileSync(new URL('./package.json', import.meta.url), 'utf8')).version;
const build = process.env.GITHUB_SHA?.slice(0, 12) ?? `local-${Date.now()}`;

/** Web版の出力に version.json を置く（更新の検知用。HTML と同じく毎回取り直す） */
function versionFile(): Plugin {
  return {
    name: 'formdoc-version',
    apply: 'build',
    generateBundle() {
      this.emitFile({ type: 'asset', fileName: 'version.json', source: JSON.stringify({ version, build }) });
    },
  };
}

// Web版とデスクトップ版は同じソースから --mode web / --mode desktop で出し分ける。
export default defineConfig(({ mode }) => {
  if (mode === 'web' && !existsSync(new URL('./src/lib/engine/pkg/formdoc_wasm.js', import.meta.url))) {
    throw new Error('Web版のエンジン（ui/src/lib/engine/pkg）がありません。先にリポジトリのルートで `bun ready` を実行してください。');
  }
  return {
    plugins: [svelte(), ...(mode === 'web' ? [versionFile()] : [])],
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
    define: {
      __APP_VERSION__: JSON.stringify(version),
      __BUILD_ID__: JSON.stringify(build),
    },
  };
});
