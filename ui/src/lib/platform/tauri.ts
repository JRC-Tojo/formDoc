// デスクトップ版の実装。エンジンは Tauri コマンド（同じ formdoc-core をネイティブ実行）。
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open, save } from '@tauri-apps/plugin-dialog';
import type { Engine, Files, Folder, Platform } from './types';

const engine: Engine = {
  catalog: () => invoke('catalog'),
  newDocument: (template) => invoke('new_document', { template }),
  codeTemplate: (template) => invoke('code_template', { template }),
  updateDocument: (doc, known) => invoke('update_document', { doc, known }),
  updateProject: (files, template, known) => invoke('update_project', { files, template, known }),
  setAsset: (path, bytes) => invoke('set_asset', bytes, { headers: { 'x-path': encodeURIComponent(path) } }),
  removeAsset: (path) => invoke('remove_asset', { path }),
  pdf: async () => new Uint8Array(await invoke<ArrayBuffer>('pdf')),
  exportTypst: () => invoke('export_typst'),
};

const files: Files = {
  async openFile(accept) {
    const path = await open({ multiple: false, filters: [{ name: accept.join(', '), extensions: accept.map((a) => a.replace(/^\./, '')) }] });
    if (!path || Array.isArray(path)) return null;
    const bytes = new Uint8Array(await invoke<ArrayBuffer>('read_file', { path }));
    return { name: path.split(/[\\/]/).pop() ?? path, path, bytes };
  },
  async saveFile(name, bytes, path) {
    const ext = name.split('.').pop() ?? '';
    const target = path ?? (await save({ defaultPath: name, filters: [{ name: ext, extensions: [ext] }] }));
    if (!target) return null;
    await invoke('write_file', bytes, { headers: { 'x-path': encodeURIComponent(target) } });
    return target;
  },
};

const folder: Folder = {
  async pick() {
    const p = await open({ directory: true, multiple: false });
    return typeof p === 'string' ? p : null;
  },
  read: (folder) => invoke('read_project', { folder }),
  write: (folder, path, text) => invoke('write_project_file', { folder, path, text }),
  openInVSCode: (folder) => invoke('open_in_vscode', { folder }),
  async watch(folder, callback) {
    await invoke('watch_project', { folder });
    const unlisten = await listen('project-changed', () => callback());
    return () => {
      unlisten();
      invoke('unwatch_project').catch(() => {});
    };
  },
};

export function createTauriPlatform(): Platform {
  return { engine, files, folder, drafts: null };
}
