// ビルド対象（--mode web / desktop）に応じて実装を切り替える。UI はこのモジュールだけを使う。
// 分岐はビルド時定数（import.meta.env.VITE_TARGET）で行い、使わない側の実装（Web版のwasm約70MBなど）を
// 出力に含めないようにする。
import type { Platform } from './types';

export { has, reason, TARGET } from './capabilities';
export type { Capability } from './capabilities';

let platform: Platform | null = null;

export async function getPlatform(): Promise<Platform> {
  if (platform) return platform;
  if (import.meta.env.VITE_TARGET === 'desktop') {
    const m = await import('./tauri');
    platform = m.createTauriPlatform();
  } else {
    const m = await import('./web');
    platform = m.createWebPlatform();
  }
  return platform;
}
