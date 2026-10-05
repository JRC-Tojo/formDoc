import { defineConfig, type Plugin } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { createHash } from 'node:crypto';
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { fontPath } from './src/lib/engine/fonts.ts';

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

/**
 * Web版の同梱フォント（library/fonts）を、版ごとのハッシュ付きの名前で配信する（wasm には埋め込まない）。
 * 開発サーバではその名前で中身を返し、ビルドでは fonts/ に出力する。名前の決め方は src/lib/engine/fonts.ts と共通。
 * 対象のファイルは crates/formdoc-library/build.rs の is_font と同じ規則（直下の otf / ttf / ttc）。
 * 開発サーバは起動時に一覧を作るので、bun ready でフォントが変わったら開発サーバを再起動する。
 */
function bundledFonts(): Plugin {
  const dir = new URL('../library/fonts/', import.meta.url);
  const load = () => {
    if (!existsSync(dir)) throw new Error('library/fonts がありません。先にリポジトリのルートで `bun ready` を実行してください。');
    return readdirSync(dir)
      .filter((f) => /\.(otf|ttf|ttc)$/i.test(f))
      .map((file) => {
        const body = readFileSync(new URL(file, dir));
        return { path: fontPath({ file, sha256: createHash('sha256').update(body).digest('hex') }), body };
      });
  };
  return {
    name: 'formdoc-fonts',
    configureServer(server) {
      const fonts = new Map(load().map((f) => [`/${f.path}`, f.body]));
      server.middlewares.use((req, res, next) => {
        const body = fonts.get((req.url ?? '').split('?')[0]);
        if (!body) return next();
        res.setHeader('Content-Type', `font/${req.url!.split('?')[0].split('.').pop()}`);
        res.end(body);
      });
    },
    generateBundle() {
      for (const f of load()) this.emitFile({ type: 'asset', fileName: f.path, source: f.body });
    },
  };
}

// Web版とデスクトップ版は同じソースから --mode web / --mode desktop で出し分ける。
export default defineConfig(({ mode }) => {
  if (mode === 'web' && !existsSync(new URL('./src/lib/engine/pkg/formdoc_wasm.js', import.meta.url))) {
    throw new Error('Web版のエンジン（ui/src/lib/engine/pkg）がありません。先にリポジトリのルートで `bun ready` を実行してください。');
  }
  return {
    plugins: [svelte(), ...(mode === 'web' ? [versionFile(), bundledFonts()] : [])],
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
