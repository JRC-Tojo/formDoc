// formdoc-core の JSON と対応する型。

export type Props = Record<string, any>;

export interface Block {
  id: string;
  kind: string;
  props: Props;
}

/** 文書情報。キーはスタイルの info.fields の key */
export type Meta = Record<string, any>;

export interface Doc {
  schema_version: number;
  library: string;
  /** スタイルの id */
  template: string;
  meta: Meta;
  blocks: Block[];
  assets: string[];
}

export type Severity = 'error' | 'warning' | 'info';

export interface Issue {
  block_id: string | null;
  field: string | null;
  severity: Severity;
  code: string;
  message: string;
  fix?: { from: string; to: string };
}

export interface VarInfo {
  name: string;
  block_id: string;
  value: number;
  text: string;
  unit: string;
  digits: number | null;
  desc: string;
}

export interface BlockResult {
  status: 'ok' | 'error' | 'ng';
  summary: string;
  vars?: string[];
  digits?: number;
  value?: number;
  /** 汎用図形の評価値（図形ごとに、繰り返しの各回） */
  shapes?: { x1: number | null; y1: number | null; x2: number | null; y2: number | null; pts?: [number | null, number | null][]; label?: string }[][];
}

export interface PageOut {
  hash: string;
  svg: string | null;
  width_pt: number;
  height_pt: number;
}

export interface Diagnostic {
  severity: 'error' | 'warning';
  message: string;
  hints: string[];
  file: string | null;
  line: number | null;
  column: number | null;
}

export interface UpdateResult {
  pages: PageOut[];
  issues: Issue[];
  vars: VarInfo[];
  blocks: Record<string, BlockResult>;
  diagnostics: Diagnostic[];
  exportable: boolean;
  compile_ms: number;
}

export interface FieldDef {
  key: string;
  label: string;
  type: string;
  required?: boolean;
  help?: string;
  options?: string[];
  default?: any;
  columns?: FieldDef[];
}

export interface ComponentDef {
  label: string;
  icon?: string;
  help?: string;
  /** 部品の分類（追加ダイアログの見出し） */
  category?: string;
  /** 置き換え先の案内。追加ダイアログには出さない（既存文書のために残している部品） */
  deprecated?: string;
  fields: FieldDef[];
}

/** 文書情報の入力欄（スタイルの info.fields） */
export interface MetaField {
  key: string;
  label: string;
  type: 'text' | 'multiline' | 'int' | 'bool' | 'date' | 'select';
  required?: boolean;
  default?: any;
  help?: string;
  options?: string[];
}

/** スタイルの規則（スタイルファイルの info） */
export interface StyleInfo {
  id: string;
  name: string;
  description: string;
  blocks: string[];
  fields: MetaField[];
  skeleton: Record<string, any>[];
  'max-heading-level': number;
  digits: Record<string, number>;
}

/** 選べるスタイル（ファイル1つ） */
export interface StyleEntry {
  /** 読み込み元（デスクトップはファイルパス、同梱は builtin:<file>、Web は browser:<file>） */
  path: string;
  source: string;
  info: StyleInfo | null;
  error?: string;
}

/** テンプレート（.fdtpl）の変数インターフェース */
export interface TemplateInput {
  name: string;
  label: string;
  unit?: string;
  /** 既定値（テンプレート内の変数定義を入力にしたもの）。無ければ挿入時に必須 */
  default?: number | null;
}
export interface TemplateExport {
  name: string;
  label: string;
}
export interface TemplateFile {
  format: 'formdoc-template';
  version: 1;
  name: string;
  description: string;
  category: string;
  /** 想定するスタイル（空なら全スタイル） */
  styles: string[];
  created: string;
  blocks: Block[];
  interface: { inputs: TemplateInput[]; exports: TemplateExport[] };
  assets: Record<string, string>;
}
/** 一覧に並ぶテンプレート */
export interface TemplateEntry {
  file: TemplateFile;
  /** 読み込み元（同梱 / このPC / フォルダのパス） */
  source: string;
  path?: string;
}

export interface Settings {
  theme: 'system' | 'light' | 'dark';
  /** UI の文字の大きさ（%） */
  fontScale: number;
  recentMax: number;
  recent: string[];
  /** テンプレートを読み込むフォルダ */
  templateFolders: string[];
  /** テンプレートの公開先（既定） */
  publishFolder: string;
}

export interface Catalog {
  library_version: string;
  styles: { file: string; source: string }[];
  snippets: TemplateFile[];
  components: Record<string, ComponentDef>;
  references: Record<string, { title: string; label?: string }>;
  fonts: string[];
}

export interface ProjectFile {
  path: string;
  text?: string;
  bytes?: number[];
}

/** 保存ファイル（.fdoc）の中身。添付ファイルは base64 で同梱する。 */
export interface SavedDoc {
  format: 'formdoc';
  version: 1 | 2;
  document: Doc;
  assets: Record<string, string>;
  /** 保存時のスタイルのソース（同じ文書なら同じPDFにするため同梱する。version 2 以降） */
  style?: { file: string; source: string };
}
