// formdoc-core の JSON と対応する型。

export type Props = Record<string, any>;

export interface Block {
  id: string;
  kind: string;
  props: Props;
}

export interface Meta {
  title: string;
  project: string;
  author: string;
  date: string;
  chapter_start: number;
  cover: boolean;
}

export interface Doc {
  schema_version: number;
  library: string;
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
  fields: FieldDef[];
}

export interface Template {
  id: string;
  name: string;
  description: string;
  blocks: string[];
  'max-heading-level': number;
  digits: Record<string, number>;
}

export interface Catalog {
  library_version: string;
  templates: Template[];
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
  version: 1;
  document: Doc;
  assets: Record<string, string>;
}
