import type { Catalog, Doc, ProjectFile, UpdateResult } from '../types';

/** 組版エンジン。Web版は Web Worker 内の wasm、デスクトップ版は Tauri コマンド（ネイティブ）。中身は同じ formdoc-core。 */
export interface Engine {
  catalog(): Promise<Catalog>;
  newDocument(template: string): Promise<Doc>;
  codeTemplate(template: string): Promise<string>;
  updateDocument(doc: Doc, known: string[]): Promise<UpdateResult>;
  updateProject(files: ProjectFile[], template: string, known: string[]): Promise<UpdateResult>;
  setAsset(path: string, bytes: Uint8Array): Promise<void>;
  removeAsset(path: string): Promise<void>;
  pdf(): Promise<Uint8Array>;
  exportTypst(): Promise<string | null>;
}

/** ファイルの入出力。 */
export interface Files {
  /** ファイルを選んで開く。キャンセル時は null */
  openFile(accept: string[]): Promise<{ name: string; path?: string; bytes: Uint8Array } | null>;
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

export interface Platform {
  engine: Engine;
  files: Files;
  folder: Folder | null;
  drafts: Drafts | null;
}
