import type { Catalog, Doc, ProjectFile, StyleInfo, UpdateResult } from '../types';

/** 組版エンジン。Web版は Web Worker 内の wasm、デスクトップ版は Tauri コマンド（ネイティブ）。中身は同じ formdoc-core。 */
export interface Engine {
  catalog(): Promise<Catalog>;
  /** スタイルの info を読む（一覧表示用。現在のスタイルは変えない） */
  styleInfo(source: string): Promise<StyleInfo>;
  /** GUIモードのスタイルを設定する */
  setStyle(source: string): Promise<StyleInfo>;
  /** 現在のスタイルで新規文書を作る */
  newDocument(): Promise<Doc>;
  /** 現在のスタイルで、コードモードの新規 main.typ を作る */
  codeTemplate(): Promise<string>;
  updateDocument(doc: Doc, known: string[]): Promise<UpdateResult>;
  updateProject(files: ProjectFile[], known: string[]): Promise<UpdateResult>;
  setAsset(path: string, bytes: Uint8Array): Promise<void>;
  removeAsset(path: string): Promise<void>;
  pdf(): Promise<Uint8Array>;
  exportTypst(): Promise<string | null>;
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
 * システムフォルダ（ユーザーごとの設定・スタイル・テンプレート）。
 * デスクトップ版は %APPDATA%\formDoc、Web版はブラウザ内（localStorage / IndexedDB）。
 */
export interface SystemStore {
  /** システムフォルダのパス（Web版は null） */
  info(): Promise<{ root: string; styles: string; templates: string; projects: string } | null>;
  loadSettings(): Promise<string | null>;
  saveSettings(text: string): Promise<void>;
  /** 利用者のスタイル（同梱スタイルは Catalog から別に得る） */
  listStyles(): Promise<TextFile[]>;
  /** スタイルを追加する（Web版のみ。デスクトップ版は styles フォルダに置く） */
  addStyle?(name: string, text: string): Promise<void>;
  /** テンプレート：このPC（システムフォルダ）と、指定フォルダから読む */
  listTemplates(folders: string[]): Promise<TextFile[]>;
  /** テンプレートを保存する。folder が null ならこのPC（システムフォルダ） */
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
