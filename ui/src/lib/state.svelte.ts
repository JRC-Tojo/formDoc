// アプリ全体の状態。文書が変わるたびにエンジンで再評価・再組版する（間引きあり）。
import { getPlatform, has } from './platform';
import type { Platform } from './platform/types';
import type {
  Block,
  Catalog,
  Doc,
  Issue,
  ProjectFile,
  SavedDoc,
  Settings,
  StyleEntry,
  StyleInfo,
  TemplateEntry,
  TemplateFile,
  UpdateResult,
} from './types';

const DRAFT_KEY = 'current';

export const DEFAULT_SETTINGS: Settings = {
  theme: 'system',
  fontScale: 100,
  recentMax: 10,
  recent: [],
  templateFolders: [],
  publishFolder: '',
};

export function uid(): string {
  return 'b' + Math.random().toString(36).slice(2, 9);
}

function clone<T>(v: T): T {
  return JSON.parse(JSON.stringify(v));
}

export function b64encode(bytes: Uint8Array): string {
  let s = '';
  for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(s);
}

export function b64decode(s: string): Uint8Array {
  const bin = atob(s);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

/** "data.2.1" のような入れ子のキーで値を取得・設定する */
function getPath(obj: any, path: string): any {
  return path.split('.').reduce((o, k) => (o == null ? o : o[k]), obj);
}
function setPath(obj: any, path: string, value: any) {
  const keys = path.split('.');
  let o = obj;
  for (const k of keys.slice(0, -1)) o = o[k];
  o[keys[keys.length - 1]] = value;
}

function fileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

/** ファイル名に使えない文字を除く */
export function safeFileName(s: string): string {
  return s.replace(/[\\/:*?"<>|\s]+/g, '_').replace(/^_+|_+$/g, '') || 'untitled';
}

function emptyDoc(): Doc {
  return { schema_version: 1, library: '', template: '', meta: {}, blocks: [], assets: [] };
}

export type Dialog =
  | { kind: 'insert' }
  | { kind: 'saveTemplate'; blockId: string }
  | { kind: 'settings' }
  | { kind: 'shapes'; blockId: string };

class AppState {
  platform: Platform | null = null;
  catalog = $state<Catalog | null>(null);
  loading = $state('エンジンを起動しています…');
  message = $state<{ kind: 'info' | 'error'; text: string } | null>(null);
  settings = $state<Settings>({ ...DEFAULT_SETTINGS });
  systemPath = $state<{ root: string; styles: string; templates: string; projects: string } | null>(null);
  dialog = $state<Dialog | null>(null);

  // スタイル
  styles = $state<StyleEntry[]>([]);
  /** 現在のスタイル（ソースと info） */
  style = $state<{ file: string; source: string; info: StyleInfo } | null>(null);

  // テンプレート
  templates = $state<TemplateEntry[]>([]);
  /** Web版で読み込んだフォルダの分（その場限り） */
  private sessionTemplates: TemplateEntry[] = [];

  mode = $state<'gui' | 'code'>('gui');
  doc = $state<Doc | null>(null);
  selectedId = $state<string | null>(null);
  result = $state<UpdateResult | null>(null);
  /** ページのハッシュ → SVG（変更のないページは再送されないため保持する） */
  svgs = new Map<string, string>();
  pageHashes = $state<string[]>([]);
  compiling = $state(false);
  dirty = $state(false);
  filePath = $state<string | null>(null);
  assets: Record<string, Uint8Array> = {};

  // コードモード
  codeText = $state('');
  codeFolder = $state<string | null>(null);
  private unwatch: (() => void) | null = null;

  private history: string[] = [];
  private future: string[] = [];
  private timer: ReturnType<typeof setTimeout> | null = null;
  private running = false;
  private queued = false;

  /** 現在のスタイルの規則（部品の一覧・見出しの階層など） */
  get template(): StyleInfo | null {
    return this.style?.info ?? null;
  }

  get selected(): Block | null {
    return this.doc?.blocks.find((b) => b.id === this.selectedId) ?? null;
  }

  issuesFor(id: string): Issue[] {
    return this.result?.issues.filter((i) => i.block_id === id) ?? [];
  }

  async init() {
    try {
      this.platform = await getPlatform();
      await this.loadSettings();
      this.catalog = await this.platform.engine.catalog();
      this.systemPath = await this.platform.system.info().catch(() => null);
      await this.loadStyles();
      this.loadTemplates();
      const startup = await this.platform.system.startupFile?.().catch(() => null);
      const draft = !startup && has('browserStorage') ? await this.platform.drafts?.load(DRAFT_KEY) : null;
      this.loading = '';
      if (startup) {
        await this.openPath(startup);
      } else if (draft) {
        await this.loadSaved(JSON.parse(draft) as SavedDoc, null);
        this.flash('前回の下書きを復元しました');
      } else {
        await this.newDocument(true);
      }
    } catch (e: any) {
      this.loading = '';
      this.flash(`起動に失敗しました: ${e?.message ?? e}`, 'error');
    }
  }

  flash(text: string, kind: 'info' | 'error' = 'info') {
    this.message = { kind, text };
    setTimeout(() => {
      if (this.message?.text === text) this.message = null;
    }, kind === 'error' ? 8000 : 3500);
  }

  // ---------- 設定 ----------

  private async loadSettings() {
    try {
      const text = await this.platform!.system.loadSettings();
      if (text) this.settings = { ...DEFAULT_SETTINGS, ...JSON.parse(text) };
    } catch {
      /* 壊れた設定は既定値で上書きする */
    }
    this.applySettings();
  }

  async saveSettings(next: Partial<Settings>) {
    this.settings = { ...this.settings, ...next };
    this.settings.recent = this.settings.recent.slice(0, Math.max(0, this.settings.recentMax));
    this.applySettings();
    await this.platform?.system.saveSettings(JSON.stringify($state.snapshot(this.settings), null, 1)).catch(() => {});
  }

  private applySettings() {
    const el = document.documentElement;
    if (this.settings.theme === 'system') delete el.dataset.theme;
    else el.dataset.theme = this.settings.theme;
    el.style.setProperty('--font-scale', String((this.settings.fontScale || 100) / 100));
  }

  private addRecent(path: string | null) {
    if (!path || !has('recentFiles')) return;
    const recent = [path, ...this.settings.recent.filter((p) => p !== path)];
    this.saveSettings({ recent });
  }

  // ---------- スタイル ----------

  /** 同梱スタイルと利用者のスタイル（システムフォルダ／ブラウザ内）を読み、info を評価する */
  async loadStyles() {
    const p = this.platform!;
    const user = await p.system.listStyles().catch(() => []);
    const entries: StyleEntry[] = user.map((f) => ({ path: f.path, source: f.text, info: null }));
    // デスクトップ版は同梱スタイルをシステムフォルダにコピー済み。Web版は同梱分を先頭に並べる
    if (!this.systemPath) {
      for (const s of this.catalog?.styles ?? []) entries.unshift({ path: `builtin:${s.file}`, source: s.source, info: null });
    }
    for (const e of entries) {
      try {
        e.info = await p.engine.styleInfo(e.source);
      } catch (err: any) {
        e.error = err?.message ?? String(err);
      }
    }
    this.styles = entries;
  }

  /** Web版：スタイルファイル（.typ）をブラウザ内に取り込む */
  async importStyle() {
    const f = await this.platform!.files.openFile(['typ']);
    if (!f) return;
    const text = new TextDecoder().decode(f.bytes);
    try {
      await this.platform!.engine.styleInfo(text);
    } catch (e: any) {
      return this.flash(e?.message ?? String(e), 'error');
    }
    await this.platform!.system.addStyle?.(f.name, text);
    await this.loadStyles();
    this.flash(`${f.name} を取り込みました`);
  }

  /**
   * スタイルを選ぶ。新規文書（部品が無い）なら骨組みと既定値で始め、
   * 執筆中の文書なら部品はそのままに、同じキーの文書情報だけを引き継ぐ。
   */
  async chooseStyle(entry: StyleEntry) {
    if (!entry.info) return;
    const p = this.platform!;
    let info: StyleInfo;
    try {
      info = await p.engine.setStyle(entry.source);
    } catch (e: any) {
      return this.flash(e?.message ?? String(e), 'error');
    }
    const file = fileName(entry.path.replace(/^(builtin|browser):/, ''));
    const fresh = await p.engine.newDocument();
    const cur = this.doc;
    if (!cur || cur.blocks.length === 0) {
      for (const b of fresh.blocks) b.id = uid();
      // スタイルを選ぶ前に入力した値は残す
      const kept = Object.fromEntries(Object.entries(cur?.meta ?? {}).filter(([k, v]) => info.fields.some((f) => f.key === k) && v !== '' && v != null));
      this.style = { file, source: entry.source, info };
      this.edit((d) => {
        d.template = fresh.template;
        d.library = fresh.library;
        d.meta = { ...fresh.meta, ...kept };
        d.blocks = fresh.blocks;
      });
    } else {
      if (this.style && !confirm(`スタイルを「${info.name}」に変えます。部品はそのまま残り、文書情報は同じ項目だけ引き継ぎます。よろしいですか？`)) return;
      this.style = { file, source: entry.source, info };
      this.edit((d) => {
        d.template = info.id;
        const meta: Record<string, any> = {};
        for (const f of info.fields) meta[f.key] = f.key in d.meta ? d.meta[f.key] : (fresh.meta[f.key] ?? null);
        d.meta = meta;
      });
    }
    await this.refresh();
  }

  // ---------- テンプレート ----------

  async loadTemplates() {
    const p = this.platform!;
    const out: TemplateEntry[] = (this.catalog?.snippets ?? []).map((file) => ({ file, source: '同梱' }));
    const files = await p.system.listTemplates(this.settings.templateFolders).catch(() => []);
    const local = this.systemPath?.templates ?? 'browser';
    for (const f of files) {
      try {
        const file = JSON.parse(f.text) as TemplateFile;
        if (file.format !== 'formdoc-template') continue;
        out.push({ file, source: f.folder === local ? 'このPC' : f.folder, path: f.path });
      } catch {
        /* テンプレートでないファイルは飛ばす */
      }
    }
    this.templates = [...out, ...this.sessionTemplates];
  }

  /** テンプレートを読み込むフォルダを追加する（デスクトップは設定に記録、Web はその場限り） */
  async addTemplateFolder() {
    const r = await this.platform!.system.pickTemplateFolder();
    if (!r) return;
    if (this.systemPath) {
      if (!this.settings.templateFolders.includes(r.folder)) await this.saveSettings({ templateFolders: [...this.settings.templateFolders, r.folder] });
    } else {
      for (const f of r.files) {
        try {
          const file = JSON.parse(f.text) as TemplateFile;
          if (file.format === 'formdoc-template') this.sessionTemplates.push({ file, source: r.folder, path: f.path });
        } catch {
          /* 飛ばす */
        }
      }
    }
    await this.loadTemplates();
    this.flash(`${r.folder} のテンプレートを読み込みました（${r.files.length} 件）`);
  }

  /** テンプレートとして保存する。folder が null ならこのPC（システムフォルダ） */
  async saveTemplate(t: TemplateFile, folder: string | null): Promise<boolean> {
    const name = `${safeFileName(t.name)}.fdtpl`;
    try {
      const path = await this.platform!.system.saveTemplate(folder, name, JSON.stringify(t, null, 1));
      await this.loadTemplates();
      this.flash(`テンプレート「${t.name}」を保存しました（${path}）`);
      return true;
    } catch (e: any) {
      this.flash(`保存できませんでした: ${e?.message ?? e}`, 'error');
      return false;
    }
  }

  /** 見出しなら配下の節ごと、それ以外はその部品だけ */
  fragmentOf(blockId: string): Block[] {
    const blocks = this.doc?.blocks ?? [];
    const i = blocks.findIndex((b) => b.id === blockId);
    if (i < 0) return [];
    const b = blocks[i];
    if (b.kind !== 'heading') return [b];
    const lv = Number(b.props.level ?? 2);
    let j = i + 1;
    while (j < blocks.length && !(blocks[j].kind === 'heading' && Number(blocks[j].props.level ?? 2) <= lv)) j++;
    return blocks.slice(i, j);
  }

  /** ブロック列（テンプレートの中身）を選択中の部品の下に入れる。添付ファイルも取り込む */
  async insertBlocks(blocks: Block[], assets: Record<string, string> = {}) {
    for (const [path, b64] of Object.entries(assets)) {
      if (this.assets[path]) continue;
      const bytes = b64decode(b64);
      this.assets[path] = bytes;
      await this.platform!.engine.setAsset(path, bytes);
    }
    const added = blocks.map((b) => ({ ...clone(b), id: uid() }));
    const afterId = this.selectedId;
    this.edit((d) => {
      const i = afterId ? d.blocks.findIndex((b) => b.id === afterId) : -1;
      d.blocks.splice(i >= 0 ? i + 1 : d.blocks.length, 0, ...added);
      for (const p of Object.keys(assets)) if (!d.assets.includes(p)) d.assets.push(p);
    });
    this.selectedId = added[0]?.id ?? this.selectedId;
  }

  // ---------- 再評価・再組版 ----------

  schedule(delay = 250) {
    if (this.timer) clearTimeout(this.timer);
    this.timer = setTimeout(() => this.refresh(), delay);
  }

  async refresh() {
    if (!this.platform) return;
    if (this.mode === 'gui' && !this.style) {
      this.result = null;
      this.pageHashes = [];
      return;
    }
    if (this.running) {
      this.queued = true;
      return;
    }
    this.running = true;
    this.compiling = true;
    try {
      const known = [...this.svgs.keys()];
      let r: UpdateResult;
      if (this.mode === 'gui') {
        if (!this.doc) return;
        r = await this.platform.engine.updateDocument($state.snapshot(this.doc) as Doc, known);
      } else {
        r = await this.platform.engine.updateProject(await this.codeFiles(), known);
      }
      for (const p of r.pages) if (p.svg) this.svgs.set(p.hash, p.svg);
      // 古いページのSVGを捨てる
      const keep = new Set(r.pages.map((p) => p.hash));
      if (r.pages.length) for (const h of [...this.svgs.keys()]) if (!keep.has(h)) this.svgs.delete(h);
      if (r.pages.length) this.pageHashes = r.pages.map((p) => p.hash);
      this.result = r;
    } catch (e: any) {
      this.flash(`組版に失敗しました: ${e?.message ?? e}`, 'error');
    } finally {
      this.running = false;
      this.compiling = false;
      if (this.queued) {
        this.queued = false;
        this.refresh();
      }
    }
  }

  // ---------- 文書の編集 ----------

  private snapshot() {
    if (!this.doc) return;
    this.history.push(JSON.stringify($state.snapshot(this.doc)));
    if (this.history.length > 200) this.history.shift();
    this.future = [];
  }

  /** 文書を変更する。fn の中で this.doc を直接書き換えてよい */
  edit(fn: (doc: Doc) => void, coalesceKey?: string) {
    if (!this.doc) return;
    // 同じ項目への連続入力は1回の取り消し単位にまとめる
    if (!coalesceKey || coalesceKey !== this.lastEditKey) this.snapshot();
    this.lastEditKey = coalesceKey ?? null;
    fn(this.doc);
    this.dirty = true;
    this.schedule();
    this.saveDraft();
  }
  private lastEditKey: string | null = null;

  undo() {
    const prev = this.history.pop();
    if (!prev || !this.doc) return;
    this.future.push(JSON.stringify($state.snapshot(this.doc)));
    this.doc = JSON.parse(prev);
    this.lastEditKey = null;
    this.schedule(0);
  }

  redo() {
    const next = this.future.pop();
    if (!next || !this.doc) return;
    this.history.push(JSON.stringify($state.snapshot(this.doc)));
    this.doc = JSON.parse(next);
    this.lastEditKey = null;
    this.schedule(0);
  }

  setProp(id: string, key: string, value: any) {
    this.edit((d) => {
      const b = d.blocks.find((x) => x.id === id);
      if (b) b.props[key] = value;
    }, `${id}:${key}`);
  }

  setMeta(key: string, value: any) {
    this.edit((d) => {
      d.meta[key] = value;
    }, `meta:${key}`);
  }

  defaultProps(kind: string): Record<string, any> {
    const def = this.catalog?.components[kind];
    const props: Record<string, any> = {};
    for (const f of def?.fields ?? []) {
      if (f.default !== undefined) props[f.key] = f.type === 'select' && f.key === 'level' ? Number(f.default) : f.default;
    }
    if (kind === 'table') props.data = { header_rows: 1, rows: [[{ text: '項目' }, { text: '値' }], [{ text: '' }, { text: '' }]] };
    if (kind === 'sum') props.items = [{ label: '', name: '', expr: '' }];
    if (kind === 'fig-shapes') props.shapes = [];
    return props;
  }

  addBlock(kind: string, afterId: string | null = this.selectedId) {
    const block: Block = { id: uid(), kind, props: this.defaultProps(kind) };
    this.edit((d) => {
      const i = afterId ? d.blocks.findIndex((b) => b.id === afterId) : -1;
      d.blocks.splice(i >= 0 ? i + 1 : d.blocks.length, 0, block);
    });
    this.selectedId = block.id;
    // 汎用図形は追加してすぐ描けるようにする
    if (kind === 'fig-shapes') this.dialog = { kind: 'shapes', blockId: block.id };
  }

  removeBlock(id: string) {
    this.edit((d) => {
      const i = d.blocks.findIndex((b) => b.id === id);
      if (i >= 0) d.blocks.splice(i, 1);
      this.selectedId = d.blocks[Math.min(i, d.blocks.length - 1)]?.id ?? null;
    });
  }

  duplicateBlock(id: string) {
    const src = this.doc?.blocks.find((b) => b.id === id);
    if (!src) return;
    const copy: Block = { id: uid(), kind: src.kind, props: clone($state.snapshot(src.props)) };
    // 変数名は重複できないため、複製時は名前に _2 を付ける
    if (typeof copy.props.name === 'string' && copy.props.name) copy.props.name += '_2';
    this.edit((d) => {
      const i = d.blocks.findIndex((b) => b.id === id);
      d.blocks.splice(i + 1, 0, copy);
    });
    this.selectedId = copy.id;
  }

  moveBlock(id: string, toIndex: number) {
    this.edit((d) => {
      const from = d.blocks.findIndex((b) => b.id === id);
      if (from < 0) return;
      const [b] = d.blocks.splice(from, 1);
      d.blocks.splice(toIndex > from ? toIndex - 1 : toIndex, 0, b);
    });
  }

  /** 1つ上（-1）・下（+1）へ移動 */
  shiftBlock(id: string, delta: number) {
    const i = this.doc?.blocks.findIndex((b) => b.id === id) ?? -1;
    if (i < 0) return;
    const to = delta < 0 ? i - 1 : i + 2;
    if (to < 0 || to > (this.doc?.blocks.length ?? 0)) return;
    this.moveBlock(id, to);
  }

  /** Lint の修正を適用する */
  applyFix(issue: Issue) {
    if (!issue.fix || !issue.block_id || !issue.field) return;
    const { from, to } = issue.fix;
    const field = issue.field;
    this.edit((d) => {
      const b = d.blocks.find((x) => x.id === issue.block_id);
      if (!b) return;
      if (field.startsWith('data.')) {
        const [, r, c] = field.split('.');
        const cell = b.props.data?.rows?.[+r]?.[+c];
        if (cell) cell.text = String(cell.text).split(from).join(to);
      } else {
        const cur = getPath(b.props, field);
        if (typeof cur === 'string') setPath(b.props, field, cur.split(from).join(to));
      }
    });
  }

  /** 同じ種類の修正を文書全体に適用する */
  applyAllFixes(code: string) {
    for (const i of this.result?.issues.filter((x) => x.code === code && x.fix) ?? []) this.applyFix(i);
  }

  // ---------- 添付ファイル ----------

  async addAsset(name: string, bytes: Uint8Array): Promise<string> {
    const safe = name.replace(/[^\w.\-]/g, '_');
    let path = `assets/${safe}`;
    let n = 1;
    while (this.assets[path]) path = `assets/${n++}_${safe}`;
    this.assets[path] = bytes;
    await this.platform!.engine.setAsset(path, bytes);
    this.edit((d) => {
      d.assets.push(path);
    });
    return path;
  }

  // ---------- 新規・保存・読込・出力 ----------

  /** 保存していない変更を破棄してよいか確かめる */
  confirmDiscard(action: string): boolean {
    return !this.dirty || confirm(`保存していない変更があります。破棄して${action}しますか？`);
  }

  /** 新規作成。文書情報の画面でスタイルを選ぶと執筆を始められる */
  async newDocument(force = false) {
    if (!force && !this.confirmDiscard('新規作成')) return;
    this.doc = emptyDoc();
    this.style = null;
    this.assets = {};
    this.filePath = null;
    this.dirty = false;
    this.history = [];
    this.future = [];
    this.selectedId = null;
    this.mode = 'gui';
    this.svgs.clear();
    this.pageHashes = [];
    this.result = null;
  }

  private saved(): SavedDoc {
    const assets: Record<string, string> = {};
    for (const [p, b] of Object.entries(this.assets)) if (this.doc?.assets.includes(p)) assets[p] = b64encode(b);
    const style = this.style ? { file: this.style.file, source: this.style.source } : undefined;
    return { format: 'formdoc', version: 2, document: $state.snapshot(this.doc!) as Doc, assets, style };
  }

  private draftTimer: ReturnType<typeof setTimeout> | null = null;
  saveDraft() {
    if (!has('browserStorage') || !this.platform?.drafts) return;
    if (this.draftTimer) clearTimeout(this.draftTimer);
    this.draftTimer = setTimeout(() => this.platform?.drafts?.save(DRAFT_KEY, JSON.stringify(this.saved())), 1000);
  }

  private async loadSaved(s: SavedDoc, path: string | null) {
    if (s.format !== 'formdoc') throw new Error('formDoc の文書ファイルではありません');
    const p = this.platform!;
    // スタイル：保存ファイルに同梱されたもの（旧形式は同じ id のスタイル）で組版する
    const sameId = this.styles.find((e) => e.info?.id === s.document.template);
    const source = s.style?.source ?? sameId?.source;
    let style: AppState['style'] = null;
    if (source) {
      try {
        const info = await p.engine.setStyle(source);
        style = { file: s.style?.file ?? `${info.id}.typ`, source, info };
      } catch (e: any) {
        this.flash(`文書のスタイルを読み込めません: ${e?.message ?? e}`, 'error');
      }
    }
    this.doc = s.document;
    this.style = style;
    this.assets = {};
    for (const [ap, b64] of Object.entries(s.assets ?? {})) {
      const bytes = b64decode(b64);
      this.assets[ap] = bytes;
      await p.engine.setAsset(ap, bytes);
    }
    this.filePath = path;
    this.dirty = false;
    this.history = [];
    this.future = [];
    this.selectedId = null;
    this.mode = 'gui';
    this.svgs.clear();
    this.pageHashes = [];
    this.result = null;
    if (style && sameId && sameId.source !== style.source) {
      this.flash(`この文書は保存時のスタイル「${style.info.name}」で組版しています（システムフォルダのスタイルとは内容が異なります。文書情報から選び直せます）`);
    } else if (!style) {
      this.flash('スタイルが見つかりません。文書情報でスタイルを選んでください', 'error');
    }
    await this.refresh();
  }

  private async openBytes(bytes: Uint8Array, name: string, path: string | null) {
    const json = JSON.parse(new TextDecoder().decode(bytes));
    // document.json（GUI文書そのもの）も開けるようにする
    const saved: SavedDoc = json.format === 'formdoc' ? json : { format: 'formdoc', version: 1, document: json, assets: {} };
    await this.loadSaved(saved, path);
    this.addRecent(path);
    this.flash(`${name} を開きました`);
  }

  async open() {
    if (!this.confirmDiscard('開き直')) return;
    const f = await this.platform!.files.openFile(['fdoc', 'json']);
    if (!f) return;
    try {
      await this.openBytes(f.bytes, f.name, f.path ?? null);
    } catch (e: any) {
      this.flash(`開けませんでした: ${e?.message ?? e}`, 'error');
    }
  }

  /** パスを指定して開く（最近使ったファイル・起動時のファイル） */
  async openPath(path: string, ask = false) {
    if (ask && !this.confirmDiscard('開き直')) return;
    const read = this.platform!.files.readPath;
    if (!read) return;
    try {
      await this.openBytes(await read(path), fileName(path), path);
    } catch (e: any) {
      this.flash(`開けませんでした: ${e?.message ?? e}`, 'error');
      // 見つからないファイルは一覧から外す
      this.saveSettings({ recent: this.settings.recent.filter((p) => p !== path) });
    }
  }

  async save(as = false) {
    if (!this.doc) return;
    const name = `${this.doc.meta.title || '文書'}.fdoc`;
    const bytes = new TextEncoder().encode(JSON.stringify(this.saved(), null, 1));
    const path = await this.platform!.files.saveFile(name, bytes, as ? undefined : (this.filePath ?? undefined));
    if (path) {
      this.filePath = has('nativeSaveDialog') ? path : null;
      this.dirty = false;
      this.addRecent(this.filePath);
      this.flash('保存しました');
    }
  }

  async exportPdf() {
    try {
      const pdf = await this.platform!.engine.pdf();
      const title = this.mode === 'gui' ? this.doc?.meta.title || '文書' : 'document';
      const path = await this.platform!.files.saveFile(`${title}.pdf`, pdf);
      if (path) this.flash('PDFを出力しました');
    } catch (e: any) {
      this.flash(e?.message ?? String(e), 'error');
    }
  }

  /** 文書（プレビューのページ）だけを印刷する。PDFと同じく、エラーがあるときは印刷しない */
  print() {
    if (!this.result?.exportable || !this.pageHashes.length) {
      this.flash('エラーを解消すると印刷できます', 'error');
      return;
    }
    window.print();
  }

  async exportTypst() {
    const src = await this.platform!.engine.exportTypst();
    if (!src) return;
    await this.platform!.files.saveFile('main.typ', new TextEncoder().encode(src));
    if (this.style) await this.platform!.files.saveFile('style.typ', new TextEncoder().encode(this.style.source));
  }

  // ---------- コードモード ----------

  private async codeFiles(): Promise<ProjectFile[]> {
    if (this.codeFolder && this.platform?.folder) return this.platform.folder.read(this.codeFolder);
    const files: ProjectFile[] = [{ path: 'main.typ', text: this.codeText }];
    if (this.style) files.push({ path: 'style.typ', text: this.style.source });
    return files;
  }

  async enterCodeMode() {
    if (!this.style) {
      this.flash('先に文書情報でスタイルを選んでください（コードモードも同じスタイルで組版します）', 'error');
      return;
    }
    this.mode = 'code';
    this.svgs.clear();
    this.pageHashes = [];
    if (!this.codeText) {
      const draft = has('browserStorage') ? await this.platform?.drafts?.load('code') : null;
      this.codeText = draft ?? (await this.platform!.engine.codeTemplate());
    }
    await this.refresh();
  }

  async enterGuiMode() {
    this.mode = 'gui';
    this.svgs.clear();
    this.pageHashes = [];
    await this.refresh();
  }

  setCode(text: string) {
    this.codeText = text;
    if (has('browserStorage')) this.platform?.drafts?.save('code', text);
    this.schedule(400);
  }

  /** GUI文書を Typst に書き出してコードモードで続ける */
  async convertToCode() {
    await this.refresh();
    const src = await this.platform!.engine.exportTypst();
    if (!src) return;
    this.codeText = src;
    this.codeFolder = null;
    await this.enterCodeMode();
    this.flash('GUI文書をTypstに変換しました（元のGUI文書は変更されません）');
  }

  async openCodeFolder() {
    const f = this.platform?.folder;
    if (!f) return;
    const dir = await f.pick();
    if (dir) await this.useCodeFolder(dir);
  }

  /** 選んだフォルダをコードモードのプロジェクトとして開き、監視を始める。main.typ・style.typ が無ければ作る */
  async useCodeFolder(dir: string) {
    const f = this.platform?.folder;
    if (!f) return;
    const files = await f.read(dir);
    if (!files.some((x) => x.path === 'main.typ')) {
      await f.write(dir, 'main.typ', this.codeText || (await this.platform!.engine.codeTemplate()));
    }
    if (!files.some((x) => x.path === 'style.typ') && this.style) await f.write(dir, 'style.typ', this.style.source);
    this.unwatch?.();
    this.codeFolder = dir;
    this.unwatch = await f.watch(dir, () => this.schedule(150));
    this.mode = 'code';
    this.svgs.clear();
    this.pageHashes = [];
    await this.refresh();
    this.flash(`${dir} を監視しています。外部エディタで保存するとプレビューが更新されます`);
  }

  async closeCodeFolder() {
    this.unwatch?.();
    this.unwatch = null;
    this.codeFolder = null;
    await this.refresh();
  }

  /** VSCodeで開く。フォルダが未選択なら、システムフォルダの projects/ に作業フォルダを作って開く */
  async openInVSCode() {
    const f = this.platform?.folder;
    if (!f) return;
    if (!this.codeFolder) {
      if (!this.style) return this.flash('先に文書情報でスタイルを選んでください', 'error');
      const base = this.systemPath?.projects;
      if (!base) return this.openCodeFolder();
      const title = safeFileName(String(this.doc?.meta.title || 'document'));
      const stamp = new Date().toISOString().slice(0, 10);
      await this.useCodeFolder(`${base}\\${title}_${stamp}`);
    }
    if (!this.codeFolder) return;
    try {
      await f.openInVSCode(this.codeFolder);
    } catch (e: any) {
      this.flash(e?.message ?? String(e), 'error');
    }
  }
}

export const app = new AppState();

// 開発ビルドでのみ、自動テストから状態を操作できるようにする（OSのダイアログは自動操作できないため）
if (import.meta.env.DEV) (globalThis as any).__formdoc = app;
