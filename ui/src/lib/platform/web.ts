// Web版の実装。エンジンは Web Worker（wasm）、保存はダウンロード、下書きは IndexedDB。
import type { Drafts, Engine, Files, Platform, SystemStore, TextFile } from './types';

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
  return {
    catalog: () => call('catalog'),
    styleInfo: (src) => call('styleInfo', src),
    setStyle: (src) => call('setStyle', src),
    newDocument: () => call('newDocument'),
    updateDocument: (doc, known) => call('updateDocument', doc, known),
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

/** Web版のシステムフォルダ相当。設定は localStorage、スタイル・テンプレートは IndexedDB。 */
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

export function createWebPlatform(): Platform {
  return { engine: workerEngine(), files, folder: null, drafts, system };
}
