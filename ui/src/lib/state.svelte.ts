// アプリ全体の状態。文書が変わるたびにエンジンで再評価・再組版する（間引きあり）。
import { getPlatform, has } from './platform';
import type { Platform } from './platform/types';
import { cloneWithIds, findBlock, flatten, locate, sectionEnd } from './tree';
import type {
  Block,
  Catalog,
  Doc,
  Issue,
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
  platform = $state.raw<Platform | null>(null);
  catalog = $state<Catalog | null>(null);
  loading = $state('エンジンを起動しています…');
  message = $state<{ kind: 'info' | 'error'; text: string } | null>(null);
  settings = $state<Settings>({ ...DEFAULT_SETTINGS });
  systemPath = $state<{ root: string; styles: string; templates: string; projects: string } | null>(null);
  dialog = $state<Dialog | null>(null);

  // 文書テンプレート
  styles = $state<StyleEntry[]>([]);
  /** 現在の文書テンプレート（ソースと info） */
  style = $state<{ file: string; source: string; info: StyleInfo } | null>(null);

  // 部品テンプレート
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

  // コードモード：同じ文書を Typst で見る。codeFolder は VSCode と同期しているフォルダ
  codeText = $state('');
  codeFolder = $state<string | null>(null);
  private unwatch: (() => void) | null = null;

  private history: string[] = [];
  private future: string[] = [];
  private timer: ReturnType<typeof setTimeout> | null = null;
  private running = false;
  private queued = false;

  /** 現在の文書テンプレートの規則（部品の一覧・見出しの階層など） */
  get template(): StyleInfo | null {
    return this.style?.info ?? null;
  }

  get selected(): Block | null {
    return findBlock(this.doc?.blocks ?? [], this.selectedId);
  }

  /** 折りたたんだ見出し・部品テンプレートのID（表示だけの状態。保存しない） */
  collapsed = $state<Record<string, boolean>>({});

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

  // ---------- 文書テンプレート ----------

  /** 同梱文書テンプレートと利用者の文書テンプレート（システムフォルダ／ブラウザ内）を読み、info を評価する */
  async loadStyles() {
    const p = this.platform!;
    const user = await p.system.listStyles().catch(() => []);
    const entries: StyleEntry[] = user.map((f) => ({ path: f.path, source: f.text, info: null }));
    // デスクトップ版は同梱文書テンプレートをシステムフォルダにコピー済み。Web版は同梱分を先頭に並べる
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

  /** Web版：文書テンプレートファイル（.typ）をブラウザ内に取り込む */
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
   * 文書テンプレートを選ぶ。新規文書（部品が無い）なら骨組みと既定値で始め、
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
      // 文書テンプレートを選ぶ前に入力した値は残す
      const kept = Object.fromEntries(Object.entries(cur?.meta ?? {}).filter(([k, v]) => info.fields.some((f) => f.key === k) && v !== '' && v != null));
      this.style = { file, source: entry.source, info };
      this.edit((d) => {
        d.template = fresh.template;
        d.library = fresh.library;
        d.meta = { ...fresh.meta, ...kept };
        d.blocks = fresh.blocks;
      });
    } else {
      if (this.style && !confirm(`文書テンプレートを「${info.name}」に変えます。部品はそのまま残り、文書情報は同じ項目だけ引き継ぎます。よろしいですか？`)) return;
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

  // ---------- 部品テンプレート ----------

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
        /* 部品テンプレートでないファイルは飛ばす */
      }
    }
    this.templates = [...out, ...this.sessionTemplates];
  }

  /** 部品テンプレートを読み込むフォルダを追加する（デスクトップは設定に記録、Web はその場限り） */
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
    this.flash(`${r.folder} の部品テンプレートを読み込みました（${r.files.length} 件）`);
  }

  /** 部品テンプレートとして保存する。folder が null ならこのPC（システムフォルダ） */
  async saveTemplate(t: TemplateFile, folder: string | null): Promise<boolean> {
    const name = `${safeFileName(t.name)}.fdtpl`;
    try {
      const path = await this.platform!.system.saveTemplate(folder, name, JSON.stringify(t, null, 1));
      await this.loadTemplates();
      this.flash(`部品テンプレート「${t.name}」を保存しました（${path}）`);
      return true;
    } catch (e: any) {
      this.flash(`保存できませんでした: ${e?.message ?? e}`, 'error');
      return false;
    }
  }

  /** 見出しなら配下の節ごと、それ以外（部品テンプレートのまとまりを含む）はその部品だけ */
  fragmentOf(blockId: string): Block[] {
    const loc = locate(this.doc?.blocks ?? [], blockId);
    if (!loc) return [];
    return loc.list.slice(loc.index, sectionEnd(loc.list, loc.index));
  }

  /** 新しい部品を入れる位置：選択中の部品の直後（同じ並び）。未選択なら文書の末尾 */
  private insertPoint(d: Doc, afterId: string | null): { list: Block[]; index: number } {
    const loc = afterId ? locate(d.blocks, afterId) : null;
    return loc ? { list: loc.list, index: loc.index + 1 } : { list: d.blocks, index: d.blocks.length };
  }

  /**
   * 部品テンプレートを選択中の部品の下に入れる。中身は1つのまとまり（group）にし、
   * 中の変数はその中だけで使えるようにする（exports の変数だけ外から使える）。
   */
  async insertTemplate(title: string, blocks: Block[], exports: string[], assets: Record<string, string> = {}) {
    for (const [path, b64] of Object.entries(assets)) {
      if (this.assets[path]) continue;
      const bytes = b64decode(b64);
      this.assets[path] = bytes;
      await this.platform!.engine.setAsset(path, bytes);
    }
    const group: Block = { id: uid(), kind: 'group', props: { title, exports }, children: blocks.map((b) => cloneWithIds(b, uid)) };
    const afterId = this.selectedId;
    this.edit((d) => {
      const at = this.insertPoint(d, afterId);
      at.list.splice(at.index, 0, group);
      for (const p of Object.keys(assets)) if (!d.assets.includes(p)) d.assets.push(p);
    });
    this.selectedId = group.id;
  }

  /** まとまりを解除して、中身を元の位置に並べる */
  ungroup(id: string) {
    this.edit((d) => {
      const loc = locate(d.blocks, id);
      if (!loc) return;
      const g = loc.list[loc.index];
      loc.list.splice(loc.index, 1, ...(g.children ?? []));
    });
  }

  // ---------- 再評価・再組版 ----------

  schedule(delay = 250) {
    if (this.timer) clearTimeout(this.timer);
    this.timer = setTimeout(() => this.refresh(), delay);
  }

  async refresh() {
    if (!this.platform) return;
    if (!this.style) {
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
      // どちらのモードでも組版するのは同じ文書（コードモードはその見え方の1つ）
      if (!this.doc) return;
      const r: UpdateResult = await this.platform.engine.updateDocument($state.snapshot(this.doc) as Doc, known);
      for (const p of r.pages) if (p.svg) this.svgs.set(p.hash, p.svg);
      // 古いページのSVGを捨てる
      const keep = new Set(r.pages.map((p) => p.hash));
      if (r.pages.length) for (const h of [...this.svgs.keys()]) if (!keep.has(h)) this.svgs.delete(h);
      if (r.pages.length) this.pageHashes = r.pages.map((p) => p.hash);
      this.result = r;
      await this.afterRefresh();
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
      const b = findBlock(d.blocks, id);
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
      const at = this.insertPoint(d, afterId);
      at.list.splice(at.index, 0, block);
    });
    this.selectedId = block.id;
    // 汎用図形は追加してすぐ描けるようにする
    if (kind === 'fig-shapes') this.dialog = { kind: 'shapes', blockId: block.id };
  }

  removeBlock(id: string) {
    this.edit((d) => {
      const loc = locate(d.blocks, id);
      if (!loc) return;
      loc.list.splice(loc.index, 1);
      const next = loc.list[Math.min(loc.index, loc.list.length - 1)] ?? loc.parent;
      this.selectedId = next?.id ?? null;
    });
  }

  duplicateBlock(id: string) {
    const src = findBlock(this.doc?.blocks ?? [], id);
    if (!src) return;
    const copy = cloneWithIds($state.snapshot(src) as Block, uid);
    // 同じ範囲に同じ名前の変数は定義できないため、複製時は名前に _2 を付ける
    if (typeof copy.props.name === 'string' && copy.props.name) copy.props.name += '_2';
    this.edit((d) => {
      const loc = locate(d.blocks, id);
      if (loc) loc.list.splice(loc.index + 1, 0, copy);
    });
    this.selectedId = copy.id;
  }

  /**
   * 部品を移動する。見出しは配下の節ごと動かす。
   * parentId: 移動先の並び（null は文書の直下、group のID ならその中）、index: 移動先の並びでの位置（移動前の数え方）
   */
  moveBlock(id: string, parentId: string | null, index: number) {
    this.edit((d) => {
      const from = locate(d.blocks, id);
      if (!from) return;
      const count = sectionEnd(from.list, from.index) - from.index;
      const moving = from.list.slice(from.index, from.index + count);
      // 自分の中（部品テンプレートの中など）へは動かせない
      if (parentId && moving.some((m) => m.id === parentId || flatten(m.children ?? []).some((c) => c.id === parentId))) return;
      const target = parentId ? findBlock(d.blocks, parentId) : null;
      const list = target ? (target.children ??= []) : d.blocks;
      let at = index;
      if (list === from.list) {
        if (at > from.index && at <= from.index + count) return; // 同じ場所
        if (at > from.index) at -= count;
      }
      from.list.splice(from.index, count);
      list.splice(Math.max(0, Math.min(at, list.length)), 0, ...moving);
    });
  }

  /** 同じ並びの中で1つ上（-1）・下（+1）へ移動（見出しは節ごと） */
  shiftBlock(id: string, delta: number) {
    const loc = locate(this.doc?.blocks ?? [], id);
    if (!loc) return;
    const parentId = loc.parent?.id ?? null;
    if (delta < 0) {
      if (loc.index === 0) return;
      // 前の要素の前へ（前が見出しの節の中身なら、その節の先頭の前へは行かない：1つ前の部品の前）
      this.moveBlock(id, parentId, loc.index - 1);
    } else {
      const end = sectionEnd(loc.list, loc.index);
      if (end >= loc.list.length) return;
      this.moveBlock(id, parentId, sectionEnd(loc.list, end) );
    }
  }

  /** Lint の修正を適用する */
  applyFix(issue: Issue) {
    if (!issue.fix || !issue.block_id || !issue.field) return;
    const { from, to } = issue.fix;
    const field = issue.field;
    this.edit((d) => {
      const b = findBlock(d.blocks, issue.block_id);
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

  /** 新規作成。文書情報の画面で文書テンプレートを選ぶと執筆を始められる */
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

  /** 新しい版への更新で閉じる最中（未保存の確認を出さない） */
  updating = false;

  /**
   * 新しい版に更新する前の準備。Web版は下書きをすぐ保存する（再読み込み後に復元される）。
   * デスクトップ版は未保存の変更があれば確認する。続けてよければ true
   */
  async prepareUpdate(): Promise<boolean> {
    if (has('browserStorage') && this.platform?.drafts) {
      if (this.draftTimer) clearTimeout(this.draftTimer);
      if (this.doc) await this.platform.drafts.save(DRAFT_KEY, JSON.stringify(this.saved()));
    } else if (this.dirty && !confirm('保存していない変更があります。更新すると失われます。続けますか？')) {
      return false;
    }
    this.updating = true;
    return true;
  }

  private async loadSaved(s: SavedDoc, path: string | null) {
    if (s.format !== 'formdoc') throw new Error('formDoc の文書ファイルではありません');
    const p = this.platform!;
    // 文書テンプレート：保存ファイルに同梱されたもの（旧形式は同じ id の文書テンプレート）で組版する
    const sameId = this.styles.find((e) => e.info?.id === s.document.template);
    const source = s.style?.source ?? sameId?.source;
    let style: AppState['style'] = null;
    if (source) {
      try {
        const info = await p.engine.setStyle(source);
        style = { file: s.style?.file ?? `${info.id}.typ`, source, info };
      } catch (e: any) {
        this.flash(`文書に同梱された文書テンプレートを読み込めません: ${e?.message ?? e}`, 'error');
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
      this.flash(`この文書は保存時の文書テンプレート「${style.info.name}」で組版しています（システムフォルダの文書テンプレートとは内容が異なります。文書情報から選び直せます）`);
    } else if (!style) {
      this.flash('文書テンプレートが見つかりません。文書情報で文書テンプレートを選んでください', 'error');
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

  // ---------- コードモード（同じ文書を Typst で見る・編集する） ----------

  /** コードの編集から文書を変えた直後（このときはエディタの内容を置き換えない） */
  private fromCode = false;
  /** VSCode 用のフォルダに最後に書いた main.typ */
  private lastWritten = '';
  private codeTimer: ReturnType<typeof setTimeout> | null = null;

  /** 組版のあと：コードモードならコードを最新にし、VSCode 用のフォルダと同期する */
  private async afterRefresh() {
    if (this.mode !== 'code' && !this.codeFolder) return;
    if (!this.fromCode) {
      const code = await this.platform!.engine.code();
      if (code != null) this.codeText = code;
    }
    this.fromCode = false;
    await this.writeFolder();
  }

  async enterCodeMode() {
    if (!this.style) {
      this.flash('先に文書情報で文書テンプレートを選んでください', 'error');
      return;
    }
    this.mode = 'code';
    this.codeText = (await this.platform!.engine.code()) ?? '';
  }

  enterGuiMode() {
    this.mode = 'gui';
  }

  /** コードを書き換えた（エディタ・外部エディタ）。少し待ってから文書に戻す */
  setCode(text: string) {
    this.codeText = text;
    if (this.codeTimer) clearTimeout(this.codeTimer);
    this.codeTimer = setTimeout(() => this.applyCode(text), 400);
  }

  private lastWarnings = '';
  private async applyCode(text: string) {
    try {
      const r = await this.platform!.engine.applyCode(text);
      const w = r.warnings.join(' / ');
      if (w && w !== this.lastWarnings) this.flash(w, 'error');
      this.lastWarnings = w;
      if (JSON.stringify(r.doc.blocks) === JSON.stringify($state.snapshot(this.doc?.blocks))) return;
      this.fromCode = true;
      this.edit((d) => {
        d.blocks = r.doc.blocks;
      }, 'code');
    } catch (e: any) {
      this.flash(e?.message ?? String(e), 'error');
    }
  }

  /** コード（main.typ）と文書テンプレート（style.typ）をファイルとして保存する */
  async saveCodeFiles() {
    const code = await this.platform!.engine.code();
    if (!code || !this.style) return;
    await this.platform!.files.saveFile('main.typ', new TextEncoder().encode(code));
    await this.platform!.files.saveFile('style.typ', new TextEncoder().encode(this.style.source));
  }

  // ---------- VSCode で開く（デスクトップ） ----------

  private async writeFolder() {
    const f = this.platform?.folder;
    if (!f || !this.codeFolder || !this.style) return;
    const code = this.mode === 'code' ? this.codeText : ((await this.platform!.engine.code()) ?? '');
    if (code && code !== this.lastWritten) {
      this.lastWritten = code;
      await f.write(this.codeFolder, 'main.typ', code);
      await f.write(this.codeFolder, 'style.typ', this.style.source);
    }
  }

  /** 外部エディタで main.typ が保存された → 文書に戻す */
  private async onFolderChanged() {
    const f = this.platform?.folder;
    if (!f || !this.codeFolder) return;
    const files = await f.read(this.codeFolder);
    const main = files.find((x) => x.path === 'main.typ')?.text;
    if (main == null || main === this.lastWritten) return;
    this.lastWritten = main;
    this.setCode(main);
  }

  /** VSCodeで開く。文書をシステムフォルダの projects/<表題>_<日付>/ に main.typ・style.typ として書き出し、保存を監視する */
  async openInVSCode() {
    const f = this.platform?.folder;
    if (!f) return;
    if (!this.style) return this.flash('先に文書情報で文書テンプレートを選んでください', 'error');
    if (!this.codeFolder) {
      const base = this.systemPath?.projects;
      if (!base) return;
      const title = safeFileName(String(this.doc?.meta.title || 'document'));
      const stamp = new Date().toISOString().slice(0, 10);
      this.codeFolder = `${base}\\${title}_${stamp}`;
      this.lastWritten = '';
      await this.writeFolder();
      this.unwatch?.();
      this.unwatch = await f.watch(this.codeFolder, () => setTimeout(() => this.onFolderChanged(), 150));
    }
    try {
      await f.openInVSCode(this.codeFolder);
      this.flash(`VSCodeで開きました。保存すると、この文書に反映されます（${this.codeFolder}）`);
    } catch (e: any) {
      this.flash(e?.message ?? String(e), 'error');
    }
  }

  async closeCodeFolder() {
    this.unwatch?.();
    this.unwatch = null;
    this.codeFolder = null;
  }
}

export const app = new AppState();

// 開発ビルドでのみ、自動テストから状態を操作できるようにする（OSのダイアログは自動操作できないため）
if (import.meta.env.DEV) (globalThis as any).__formdoc = app;
