// アプリ全体の状態。文書が変わるたびにエンジンで再評価・再組版する（間引きあり）。
import { getPlatform, has } from './platform';
import type { Platform } from './platform/types';
import type { Block, Catalog, Doc, Issue, ProjectFile, SavedDoc, UpdateResult } from './types';

const DRAFT_KEY = 'current';

function uid(): string {
  return 'b' + Math.random().toString(36).slice(2, 9);
}

function clone<T>(v: T): T {
  return JSON.parse(JSON.stringify(v));
}

function b64encode(bytes: Uint8Array): string {
  let s = '';
  for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(s);
}

function b64decode(s: string): Uint8Array {
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

class AppState {
  platform: Platform | null = null;
  catalog = $state<Catalog | null>(null);
  loading = $state('エンジンを起動しています…');
  message = $state<{ kind: 'info' | 'error'; text: string } | null>(null);

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
  codeTemplateId = $state('keisansho');
  private unwatch: (() => void) | null = null;

  private history: string[] = [];
  private future: string[] = [];
  private timer: ReturnType<typeof setTimeout> | null = null;
  private running = false;
  private queued = false;

  get template() {
    return this.catalog?.templates.find((t) => t.id === this.doc?.template) ?? null;
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
      this.catalog = await this.platform.engine.catalog();
      const draft = has('browserStorage') ? await this.platform.drafts?.load(DRAFT_KEY) : null;
      if (draft) {
        await this.loadSaved(JSON.parse(draft) as SavedDoc, null);
        this.flash('前回の下書きを復元しました');
      } else {
        await this.newDocument('keisansho');
      }
      this.loading = '';
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

  // ---------- 再評価・再組版 ----------

  schedule(delay = 250) {
    if (this.timer) clearTimeout(this.timer);
    this.timer = setTimeout(() => this.refresh(), delay);
  }

  async refresh() {
    if (!this.platform) return;
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
        r = await this.platform.engine.updateProject(await this.codeFiles(), this.codeTemplateId, known);
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
      (d.meta as any)[key] = value;
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
    if (kind === 'fig-shapes') props.shapes = [{ kind: 'rect', x1: '0', y1: '0', x2: '4', y2: '2' }];
    return props;
  }

  addBlock(kind: string, afterId: string | null = this.selectedId) {
    const block: Block = { id: uid(), kind, props: this.defaultProps(kind) };
    this.edit((d) => {
      const i = afterId ? d.blocks.findIndex((b) => b.id === afterId) : -1;
      d.blocks.splice(i >= 0 ? i + 1 : d.blocks.length, 0, block);
    });
    this.selectedId = block.id;
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

  async newDocument(template: string) {
    const doc = await this.platform!.engine.newDocument(template);
    for (const b of doc.blocks) b.id = uid();
    this.doc = doc;
    this.assets = {};
    this.filePath = null;
    this.dirty = false;
    this.history = [];
    this.future = [];
    this.selectedId = null;
    this.mode = 'gui';
    this.svgs.clear();
    this.pageHashes = [];
    await this.refresh();
  }

  private saved(): SavedDoc {
    const assets: Record<string, string> = {};
    for (const [p, b] of Object.entries(this.assets)) if (this.doc?.assets.includes(p)) assets[p] = b64encode(b);
    return { format: 'formdoc', version: 1, document: $state.snapshot(this.doc!) as Doc, assets };
  }

  private draftTimer: ReturnType<typeof setTimeout> | null = null;
  saveDraft() {
    if (!has('browserStorage') || !this.platform?.drafts) return;
    if (this.draftTimer) clearTimeout(this.draftTimer);
    this.draftTimer = setTimeout(() => this.platform?.drafts?.save(DRAFT_KEY, JSON.stringify(this.saved())), 1000);
  }

  private async loadSaved(s: SavedDoc, path: string | null) {
    if (s.format !== 'formdoc') throw new Error('formDoc の文書ファイルではありません');
    this.doc = s.document;
    this.assets = {};
    for (const [p, b64] of Object.entries(s.assets ?? {})) {
      const bytes = b64decode(b64);
      this.assets[p] = bytes;
      await this.platform!.engine.setAsset(p, bytes);
    }
    this.filePath = path;
    this.dirty = false;
    this.history = [];
    this.future = [];
    this.selectedId = null;
    this.mode = 'gui';
    this.svgs.clear();
    this.pageHashes = [];
    await this.refresh();
  }

  async open() {
    const f = await this.platform!.files.openFile(['fdoc', 'json']);
    if (!f) return;
    try {
      const text = new TextDecoder().decode(f.bytes);
      const json = JSON.parse(text);
      // document.json（GUI文書そのもの）も開けるようにする
      const saved: SavedDoc = json.format === 'formdoc' ? json : { format: 'formdoc', version: 1, document: json, assets: {} };
      await this.loadSaved(saved, f.path ?? null);
      this.flash(`${f.name} を開きました`);
    } catch (e: any) {
      this.flash(`開けませんでした: ${e?.message ?? e}`, 'error');
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

  async exportTypst() {
    const src = await this.platform!.engine.exportTypst();
    if (!src) return;
    await this.platform!.files.saveFile('main.typ', new TextEncoder().encode(src));
  }

  // ---------- コードモード ----------

  private async codeFiles(): Promise<ProjectFile[]> {
    if (this.codeFolder && this.platform?.folder) return this.platform.folder.read(this.codeFolder);
    return [{ path: 'main.typ', text: this.codeText }];
  }

  async enterCodeMode() {
    this.mode = 'code';
    this.svgs.clear();
    this.pageHashes = [];
    if (!this.codeText) {
      const draft = has('browserStorage') ? await this.platform?.drafts?.load('code') : null;
      this.codeText = draft ?? (await this.platform!.engine.codeTemplate(this.codeTemplateId));
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

  /** 選んだフォルダをコードモードのプロジェクトとして開き、監視を始める */
  async useCodeFolder(dir: string) {
    const f = this.platform?.folder;
    if (!f) return;
    const files = await f.read(dir);
    if (!files.some((x) => x.path === 'main.typ')) {
      await f.write(dir, 'main.typ', this.codeText || (await this.platform!.engine.codeTemplate(this.codeTemplateId)));
    }
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

  async openInVSCode() {
    if (!this.codeFolder) await this.openCodeFolder();
    if (this.codeFolder) await this.platform?.folder?.openInVSCode(this.codeFolder);
  }
}

export const app = new AppState();

// 開発ビルドでのみ、自動テストから状態を操作できるようにする（OSのダイアログは自動操作できないため）
if (import.meta.env.DEV) (globalThis as any).__formdoc = app;
