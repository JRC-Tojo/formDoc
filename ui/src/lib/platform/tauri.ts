// デスクトップ版の実装。エンジンは Tauri コマンド（同じ formdoc-core をネイティブ実行）。
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open, save } from '@tauri-apps/plugin-dialog';
import type { Engine, Files, Folder, Platform, SystemStore, TextFile } from './types';

const engine: Engine = {
  catalog: () => invoke('catalog'),
  styleInfo: (source) => invoke('style_info', { source }),
  setStyle: (source) => invoke('set_style', { source }),
  newDocument: () => invoke('new_document'),
  codeTemplate: () => invoke('code_template'),
  updateDocument: (doc, known) => invoke('update_document', { doc, known }),
  updateProject: (files, known) => invoke('update_project', { files, known }),
  setAsset: (path, bytes) => invoke('set_asset', bytes, { headers: { 'x-path': encodeURIComponent(path) } }),
  removeAsset: (path) => invoke('remove_asset', { path }),
  pdf: async () => new Uint8Array(await invoke<ArrayBuffer>('pdf')),
  exportTypst: () => invoke('export_typst'),
};

const readPath = async (path: string) => new Uint8Array(await invoke<ArrayBuffer>('read_file', { path }));

const files: Files = {
  async openFile(accept) {
    const path = await open({ multiple: false, filters: [{ name: accept.join(', '), extensions: accept.map((a) => a.replace(/^\./, '')) }] });
    if (!path || Array.isArray(path)) return null;
    return { name: path.split(/[\\/]/).pop() ?? path, path, bytes: await readPath(path) };
  },
  readPath,
  async saveFile(name, bytes, path) {
    const ext = name.split('.').pop() ?? '';
    const target = path ?? (await save({ defaultPath: name, filters: [{ name: ext, extensions: [ext] }] }));
    if (!target) return null;
    await invoke('write_file', bytes, { headers: { 'x-path': encodeURIComponent(target) } });
    return target;
  },
};

async function pickDir(): Promise<string | null> {
  const p = await open({ directory: true, multiple: false });
  return typeof p === 'string' ? p : null;
}

const folder: Folder = {
  pick: pickDir,
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

type SysInfo = { root: string; styles: string; templates: string; projects: string };
let sysInfo: Promise<SysInfo> | null = null;
const info = () => (sysInfo ??= invoke<SysInfo>('system_info'));

const system: SystemStore = {
  info,
  loadSettings: () => invoke('read_settings'),
  saveSettings: (text) => invoke('write_settings', { text }),
  listStyles: () => invoke('list_styles'),
  listTemplates: (folders) => invoke('list_templates', { folders }),
  async saveTemplate(dir, name, text) {
    const target = dir ?? (await info()).templates;
    await invoke('write_project_file', { folder: target, path: name, text });
    return `${target}\\${name}`;
  },
  async pickTemplateFolder() {
    const dir = await pickDir();
    if (!dir) return null;
    const files = await invoke<TextFile[]>('list_templates', { folders: [dir] });
    // list_templates はシステムフォルダ分も返すため、選んだフォルダの分だけにする
    return { folder: dir, files: files.filter((f) => f.folder === dir) };
  },
  openPath: (path) => invoke('open_path', { path }),
  startupFile: () => invoke('startup_file'),
};

export function createTauriPlatform(): Platform {
  return { engine, files, folder, drafts: null, system };
}
