// scripts/*.ts の共通処理（bun で実行する）。
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';

export const root = path.resolve(import.meta.dir, '..');

/** コマンドを実行する。失敗したら例外。出力はそのまま画面に出す */
export function run(cmd: string[], opts: { cwd?: string } = {}) {
  const p = Bun.spawnSync(cmd, { cwd: opts.cwd ?? root, stdio: ['inherit', 'inherit', 'inherit'] });
  if (p.exitCode !== 0) throw new Error(`失敗しました（終了コード ${p.exitCode}）: ${cmd.join(' ')}`);
}

/** コマンドの標準出力を返す。コマンドが無い・失敗したときは null */
export function output(cmd: string[]): string | null {
  try {
    const p = Bun.spawnSync(cmd, { cwd: root, stdout: 'pipe', stderr: 'ignore' });
    return p.exitCode === 0 ? p.stdout.toString().trim() : null;
  } catch {
    return null;
  }
}

/** Cargo.lock に記録された依存クレートの版（wasm-bindgen-cli の版をそろえるため） */
export function lockedVersion(crate: string): string {
  const lock = readFileSync(path.join(root, 'Cargo.lock'), 'utf8');
  const m = lock.match(new RegExp(`\\[\\[package\\]\\]\\r?\\nname = "${crate}"\\r?\\nversion = "([^"]+)"`));
  if (!m) throw new Error(`Cargo.lock に ${crate} がありません`);
  return m[1];
}

/** 2つのファイルの中身が同じか */
export function sameFile(a: string, b: string): boolean {
  return existsSync(a) && existsSync(b) && Buffer.compare(readFileSync(a), readFileSync(b)) === 0;
}

export const step = (msg: string) => console.log(`\n▶ ${msg}`);
