// Web版：formdoc-core（wasm）を Web Worker 内で動かす。UIスレッドを止めないため。
// 最初のメッセージ init でサイトの基準 URL を受け取り、同梱フォントを取得して wasm に渡してから、ほかの要求に応える。
import init, * as wasm from './pkg/formdoc_wasm.js';
import { fontPath, type FontFile } from './fonts';

type Req = { id: number; method: string; args: any[] };

/** 起動処理（wasm の初期化とフォントの読み込み）。結果は利用者に知らせる警告 */
let ready: Promise<string[]> | null = null;

const hex = (buf: ArrayBuffer) => [...new Uint8Array(buf)].map((b) => b.toString(16).padStart(2, '0')).join('');

/** 同梱フォントを取得して照合し、wasm に渡す。版の違うフォントが混ざると同じ文書でもPDFが変わるため、不一致は警告する */
async function start(base: string): Promise<string[]> {
  await init();
  const list: FontFile[] = JSON.parse(wasm.font_files());
  const warnings: string[] = [];
  // 安全な接続（https・localhost）でないと crypto.subtle が使えず、照合できない
  if (!crypto.subtle) warnings.push('安全な接続（https）で開いていないため、フォントの版を照合できません。');
  const bodies = await Promise.all(
    list.map(async (f) => {
      const res = await fetch(new URL(fontPath(f), base));
      if (!res.ok) throw new Error(`フォント ${f.file} を取得できません（HTTP ${res.status}）。ページを再読み込みしてください`);
      const body = new Uint8Array(await res.arrayBuffer());
      const actual = crypto.subtle ? hex(await crypto.subtle.digest('SHA-256', body)) : f.sha256;
      if (actual !== f.sha256) warnings.push(`フォント ${f.file} の版がエンジンと一致しません。ページを再読み込みしてください（出力PDFが他の環境と変わる可能性があります）。`);
      return body;
    }),
  );
  for (const b of bodies) wasm.add_font(b);
  wasm.init_fonts();
  return warnings;
}

self.onmessage = async (ev: MessageEvent<Req>) => {
  const { id, method, args } = ev.data;
  try {
    if (method === 'init') {
      ready = start(args[0]);
      await ready;
      (self as any).postMessage({ id, ok: true, result: null });
      return;
    }
    if (!ready) throw new Error('エンジンが初期化されていません');
    const warnings = await ready;
    let result: any;
    switch (method) {
      case 'catalog':
        result = { ...JSON.parse(wasm.catalog()), warnings };
        break;
      case 'styleInfo':
        result = JSON.parse(wasm.style_info(args[0]));
        break;
      case 'setStyle':
        result = JSON.parse(wasm.set_style(args[0]));
        break;
      case 'newDocument':
        result = JSON.parse(wasm.new_document());
        break;
      case 'updateDocument':
        result = JSON.parse(wasm.update_document(JSON.stringify(args[0]), JSON.stringify(args[1])));
        break;
      case 'code':
        result = wasm.code() ?? null;
        break;
      case 'applyCode':
        result = JSON.parse(wasm.apply_code(args[0]));
        break;
      case 'setAsset':
        wasm.set_asset(args[0], args[1]);
        break;
      case 'removeAsset':
        wasm.remove_asset(args[0]);
        break;
      case 'pdf': {
        const bytes = wasm.pdf();
        (self as any).postMessage({ id, ok: true, result: bytes }, [bytes.buffer]);
        return;
      }
      default:
        throw new Error(`unknown method ${method}`);
    }
    (self as any).postMessage({ id, ok: true, result });
  } catch (e: any) {
    (self as any).postMessage({ id, ok: false, error: e?.message ?? String(e) });
  }
};
