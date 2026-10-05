// Web版の同梱フォントの配信名。wasm にはフォントを埋め込まず、Worker が起動時に取得して渡す。
// 版ごとに名前が変わるよう SHA-256 の先頭を付ける（ブラウザのキャッシュに古い版が残っても混ざらない）。
// vite.config.ts（配信物の出力）と worker.ts（取得）の両方がこの関数で名前を決める。

/** 同梱フォント1つ（formdoc-core の api::font_files と同じ形） */
export interface FontFile {
  file: string;
  sha256: string;
  size: number;
}

/** 配信物の中のパス（サイトの基準 URL からの相対）。例：fonts/NotoSerifJP-Regular.2c9a12dbd4f2408c.otf */
export function fontPath(f: Pick<FontFile, 'file' | 'sha256'>): string {
  const dot = f.file.lastIndexOf('.');
  const [stem, ext] = dot < 0 ? [f.file, ''] : [f.file.slice(0, dot), f.file.slice(dot)];
  return `fonts/${stem}.${f.sha256.slice(0, 16)}${ext}`;
}
