// Web版の実装。エンジンは Web Worker（wasm）、保存はダウンロード、下書きは IndexedDB。
import type { Drafts, Engine, Files, Platform } from './types';

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
    newDocument: (t) => call('newDocument', t),
    codeTemplate: (t) => call('codeTemplate', t),
    updateDocument: (doc, known) => call('updateDocument', doc, known),
    updateProject: (files, t, known) => call('updateProject', files, t, known),
    setAsset: (path, bytes) => call('setAsset', path, bytes),
    removeAsset: (path) => call('removeAsset', path),
    pdf: () => call('pdf'),
    exportTypst: () => call('exportTypst'),
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

function idb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open('formdoc', 1);
    req.onupgradeneeded = () => req.result.createObjectStore('drafts');
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

export function createWebPlatform(): Platform {
  return { engine: workerEngine(), files, folder: null, drafts };
}
