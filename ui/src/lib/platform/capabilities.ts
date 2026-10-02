// 能力フラグ。Web版とデスクトップ版のどちらかでしか使えない機能は、ここで宣言して UI 側で <Gate> により無効化する。

export type Capability =
  /** ローカルのフォルダを開いて読み書きする（コードモードのプロジェクト） */
  | 'localFolder'
  /** 「VSCodeで開く」 */
  | 'openInVSCode'
  /** フォルダの変更監視（外部エディタで保存→自動でプレビュー更新） */
  | 'fileWatch'
  /** OS の保存ダイアログで保存先を選ぶ */
  | 'nativeSaveDialog'
  /** ブラウザ内への自動保存（IndexedDB） */
  | 'browserStorage'
  /** 最近使ったファイルをパスで開き直す */
  | 'recentFiles'
  /** 共有フォルダの社内ライブラリを参照（未実装：両版とも無効） */
  | 'sharedLibraryFolder';

export const TARGET: 'web' | 'desktop' =
  import.meta.env.VITE_TARGET === 'desktop'
    ? 'desktop'
    : 'web';

const TABLE: Record<Capability, { web: boolean; desktop: boolean; reason: string }> = {
  localFolder: { web: false, desktop: true, reason: 'デスクトップ版でのみ利用できます' },
  openInVSCode: { web: false, desktop: true, reason: 'デスクトップ版でのみ利用できます' },
  fileWatch: { web: false, desktop: true, reason: 'デスクトップ版でのみ利用できます' },
  nativeSaveDialog: { web: false, desktop: true, reason: 'デスクトップ版でのみ利用できます' },
  browserStorage: { web: true, desktop: false, reason: 'Web版でのみ利用できます' },
  recentFiles: { web: false, desktop: true, reason: 'Web版ではファイルをパスで開き直せないため、デスクトップ版でのみ利用できます' },
  sharedLibraryFolder: { web: false, desktop: false, reason: '準備中の機能です' },
};

export function has(cap: Capability): boolean {
  return TABLE[cap][TARGET];
}

export function reason(cap: Capability): string {
  return TABLE[cap].reason;
}
