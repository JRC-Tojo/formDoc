import type { Catalog, Doc, ProjectFile, StyleInfo, UpdateResult } from '../types';

/** コードモードの編集を文書に戻した結果 */
export interface CodeApplied {
  doc: Doc;
  warnings: string[];
}

/** 組版エンジン。Web版は Web Worker 内の wasm、デスクトップ版は Tauri コマンド（ネイティブ）。中身は同じ formdoc-core。 */
export interface Engine {
  catalog(): Promise<Catalog>;
  /** 文書テンプレートの info を読む（一覧表示用。現在の文書テンプレートは変えない） */
  styleInfo(source: string): Promise<StyleInfo>;
  /** GUIモードの文書テンプレートを設定する */
  setStyle(source: string): Promise<StyleInfo>;
  /** 現在の文書テンプレートで新規文書を作る */
  newDocument(): Promise<Doc>;
  updateDocument(doc: Doc, known: string[]): Promise<UpdateResult>;
  /** コードモードで見せるコード（直前に組版した文書を、部品ごとの目印つきの Typst にしたもの） */
  code(): Promise<string | null>;
  /** コードモードの編集を、直前に組版した文書に戻す（変わった部品は Typstコード部品になる） */
  applyCode(code: string): Promise<CodeApplied>;
  setAsset(path: string, bytes: Uint8Array): Promise<void>;
  removeAsset(path: string): Promise<void>;
  pdf(): Promise<Uint8Array>;
}

/** ファイルの入出力。 */
export interface Files {
  /** ファイルを選んで開く。キャンセル時は null */
  openFile(accept: string[]): Promise<{ name: string; path?: string; bytes: Uint8Array } | null>;
  /** パスを指定して読む（デスクトップのみ。最近使ったファイル・起動時のファイル） */
  readPath?(path: string): Promise<Uint8Array>;
  /** 保存する（Web はダウンロード、デスクトップは保存ダイアログ）。保存先パスを返す */
  saveFile(suggestedName: string, bytes: Uint8Array, path?: string): Promise<string | null>;
}

/** コードモードのプロジェクトフォルダ（デスクトップ版のみ）。 */
export interface Folder {
  pick(): Promise<string | null>;
  read(folder: string): Promise<ProjectFile[]>;
  write(folder: string, path: string, text: string): Promise<void>;
  openInVSCode(folder: string): Promise<void>;
  /** 変更を監視し、変化があれば callback。戻り値で監視解除 */
  watch(folder: string, callback: () => void): Promise<() => void>;
}

/** 下書きの自動保存（Web版のみ）。 */
export interface Drafts {
  load(key: string): Promise<string | null>;
  save(key: string, value: string): Promise<void>;
}

/** 読み込んだテキストファイル */
export interface TextFile {
  path: string;
  /** 読み込んだフォルダ */
  folder: string;
  text: string;
}

/**
 * システムフォルダ（ユーザーごとの設定・文書テンプレート・部品テンプレート）。
 * デスクトップ版は %APPDATA%\formDoc、Web版はブラウザ内（localStorage / IndexedDB）。
 */
export interface SystemStore {
  /** システムフォルダのパス（Web版は null） */
  info(): Promise<{ root: string; styles: string; templates: string; projects: string } | null>;
  loadSettings(): Promise<string | null>;
  saveSettings(text: string): Promise<void>;
  /** 利用者の文書テンプレート（同梱文書テンプレートは Catalog から別に得る） */
  listStyles(): Promise<TextFile[]>;
  /** 文書テンプレートを追加する（Web版のみ。デスクトップ版は styles フォルダに置く） */
  addStyle?(name: string, text: string): Promise<void>;
  /** 部品テンプレート：このPC（システムフォルダ）と、指定フォルダから読む */
  listTemplates(folders: string[]): Promise<TextFile[]>;
  /** 部品テンプレートを保存する。folder が null ならこのPC（システムフォルダ） */
  saveTemplate(folder: string | null, name: string, text: string): Promise<string>;
  /** フォルダを選ぶ（デスクトップ：パスを返す。Web：中の .fdtpl を読み込んで返す） */
  pickTemplateFolder(): Promise<{ folder: string; files: TextFile[] } | null>;
  /** フォルダをエクスプローラーで開く（デスクトップのみ） */
  openPath?(path: string): Promise<void>;
  /** 起動時に渡されたファイル（デスクトップ：.fdoc のダブルクリック） */
  startupFile?(): Promise<string | null>;
}

export interface Platform {
  engine: Engine;
  files: Files;
  folder: Folder | null;
  drafts: Drafts | null;
  system: SystemStore;
}
