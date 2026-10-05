// Web版の実装。エンジンは Web Worker（wasm）、保存はダウンロード、下書きは IndexedDB。
import type { Drafts, Engine, Files, Platform, SystemStore, TextFile, Updater } from './types';

function workerEngine(): Engine {
  const worker = new Worker(new URL('../engine/worker.ts', import.meta.url), { type: 'module' });
  let seq = 0;
  const pending = new Map<number, { resolve: (v: any) => void; reject: (e: any) => void }>();
  worker.onmessage = (ev) => {
    const { id, ok, result, error } = ev.data;
    const p = pending.get(id);
    if (!p) return;
    pending.delete(id);
    ok ? p.resolve(result) : p.reject(new Error(error));
  };
  const call = (method: string, ...args: any[]) =>
    new Promise<any>((resolve, reject) => {
      const id = ++seq;
      pending.set(id, { resolve, reject });
      worker.postMessage({ id, method, args });
    });
  // 最初にサイトの基準 URL を渡す（同梱フォントの取得先。GitHub Pages のサブパス配信でも届くように）。
  // Worker は届いた順に処理するので、続く要求は初期化の後に処理される
  call('init', new URL('./', document.baseURI).href).catch(() => {});
  return {
    catalog: () => call('catalog'),
    styleInfo: (src) => call('styleInfo', src),
    setStyle: (src) => call('setStyle', src),
    newDocument: () => call('newDocument'),
    updateDocument: (doc, known) => call('updateDocument', doc, known),
    completeChapters: (doc) => call('completeChapters', doc),
    code: () => call('code'),
    applyCode: (code) => call('applyCode', code),
    setAsset: (path, bytes) => call('setAsset', path, bytes),
    removeAsset: (path) => call('removeAsset', path),
    pdf: () => call('pdf'),
  };
}

const files: Files = {
  openFile(accept) {
    return new Promise((resolve) => {
      const input = document.createElement('input');
      input.type = 'file';
      input.accept = accept.map((a) => (a.startsWith('.') ? a : `.${a}`)).join(',');
      input.onchange = async () => {
        const f = input.files?.[0];
        if (!f) return resolve(null);
        resolve({ name: f.name, bytes: new Uint8Array(await f.arrayBuffer()) });
      };
      input.addEventListener('cancel', () => resolve(null));
      input.click();
    });
  },
  async saveFile(name, bytes) {
    const blob = new Blob([bytes as BlobPart]);
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = name;
    a.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
    return name;
  },
};

const STORES = ['drafts', 'styles', 'templates'] as const;

function idb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open('formdoc', 2);
    req.onupgradeneeded = () => {
      for (const s of STORES) if (!req.result.objectStoreNames.contains(s)) req.result.createObjectStore(s);
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

const drafts: Drafts = {
  async load(key) {
    try {
      const db = await idb();
      return await new Promise((resolve) => {
        const r = db.transaction('drafts').objectStore('drafts').get(key);
        r.onsuccess = () => resolve((r.result as string) ?? null);
        r.onerror = () => resolve(null);
      });
    } catch {
      return null;
    }
  },
  async save(key, value) {
    try {
      const db = await idb();
      db.transaction('drafts', 'readwrite').objectStore('drafts').put(value, key);
    } catch {
      /* 保存できない環境（プライベートウィンドウ等）では下書きを保持しない */
    }
  },
};

async function idbAll(store: string): Promise<[string, string][]> {
  try {
    const db = await idb();
    return await new Promise((resolve) => {
      const out: [string, string][] = [];
      const r = db.transaction(store).objectStore(store).openCursor();
      r.onsuccess = () => {
        const c = r.result;
        if (!c) return resolve(out);
        out.push([String(c.key), String(c.value)]);
        c.continue();
      };
      r.onerror = () => resolve(out);
    });
  } catch {
    return [];
  }
}

async function idbPut(store: string, key: string, value: string) {
  const db = await idb();
  await new Promise<void>((resolve, reject) => {
    const tx = db.transaction(store, 'readwrite');
    tx.objectStore(store).put(value, key);
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
}

const SETTINGS_KEY = 'formdoc.settings';

/** Web版のシステムフォルダ相当。設定は localStorage、文書テンプレート・部品テンプレートは IndexedDB。 */
const system: SystemStore = {
  info: async () => null,
  async loadSettings() {
    try {
      return localStorage.getItem(SETTINGS_KEY);
    } catch {
      return null;
    }
  },
  async saveSettings(text) {
    try {
      localStorage.setItem(SETTINGS_KEY, text);
    } catch {
      /* 保存できない環境では既定値で動く */
    }
  },
  async listStyles() {
    return (await idbAll('styles')).map(([k, v]) => ({ path: `browser:${k}`, folder: 'browser', text: v }));
  },
  addStyle: (name, text) => idbPut('styles', name, text),
  async listTemplates() {
    return (await idbAll('templates')).map(([k, v]) => ({ path: `browser:${k}`, folder: 'browser', text: v }));
  },
  async saveTemplate(folder, name, text) {
    if (folder === null) {
      await idbPut('templates', name, text);
      return `browser:${name}`;
    }
    // 公開先：Web版はフォルダに書き込めないため、ダウンロードして利用者が置く
    await files.saveFile(name, new TextEncoder().encode(text));
    return name;
  },
  pickTemplateFolder() {
    return new Promise((resolve) => {
      const input = document.createElement('input');
      input.type = 'file';
      (input as any).webkitdirectory = true;
      input.onchange = async () => {
        const list = [...(input.files ?? [])].filter((f) => f.name.toLowerCase().endsWith('.fdtpl'));
        const first = input.files?.[0] as any;
        const folder = first?.webkitRelativePath?.split('/')[0] ?? 'フォルダ';
        const out: TextFile[] = [];
        for (const f of list) out.push({ path: (f as any).webkitRelativePath || f.name, folder, text: await f.text() });
        resolve({ folder, files: out });
      };
      input.addEventListener('cancel', () => resolve(null));
      input.click();
    });
  },
};

/**
 * 新しい版の検知（main へのマージのたびに GitHub Pages へ配信される）。
 * 配信中の version.json のビルド識別子が、いま動いているものと違えば新しい版。
 * JS・wasm はファイル名にハッシュが付くため、HTML さえ新しくなれば中身はすべて新しくなる。
 * HTML はブラウザ・配信側（GitHub Pages は最大10分）にキャッシュされうるため、再読み込みでは
 * URL に ?v=<識別子> を付けて別物として取り直させる（付けた ?v= は起動時に消す）。
 */
async function latest(): Promise<{ version: string; build: string } | null> {
  const res = await fetch(`./version.json?t=${Date.now()}`, { cache: 'no-store' });
  return res.ok ? res.json() : null;
}

const updater: Updater = {
  interval: 10 * 60 * 1000,
  async check() {
    const v = await latest();
    return v?.build && v.build !== __BUILD_ID__ ? { id: v.build, version: v.version } : null;
  },
  async apply() {
    const v = await latest();
    location.replace(`${location.pathname}?v=${encodeURIComponent(v?.build ?? Date.now())}`);
  },
};

export function createWebPlatform(): Platform {
  // 更新時に付けた ?v= を表示から消す（ブックマーク等に残さないため）
  const url = new URL(location.href);
  if (url.searchParams.has('v')) {
    url.searchParams.delete('v');
    history.replaceState(history.state, '', url);
  }
  return { engine: workerEngine(), files, folder: null, drafts, system, updater: import.meta.env.DEV ? null : updater };
}
