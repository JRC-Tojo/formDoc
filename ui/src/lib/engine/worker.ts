// Web版：formdoc-core（wasm）を Web Worker 内で動かす。UIスレッドを止めないため。
import init, * as wasm from './pkg/formdoc_wasm.js';

const ready = init();

type Req = { id: number; method: string; args: any[] };

self.onmessage = async (ev: MessageEvent<Req>) => {
  const { id, method, args } = ev.data;
  try {
    await ready;
    let result: any;
    switch (method) {
      case 'catalog':
        result = JSON.parse(wasm.catalog());
        break;
      case 'newDocument':
        result = JSON.parse(wasm.new_document(args[0]));
        break;
      case 'codeTemplate':
        result = wasm.code_template(args[0]);
        break;
      case 'updateDocument':
        result = JSON.parse(wasm.update_document(JSON.stringify(args[0]), JSON.stringify(args[1])));
        break;
      case 'updateProject':
        result = JSON.parse(wasm.update_project(JSON.stringify(args[0]), args[1], JSON.stringify(args[2])));
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
      case 'exportTypst':
        result = wasm.export_typst() ?? null;
        break;
      default:
        throw new Error(`unknown method ${method}`);
    }
    (self as any).postMessage({ id, ok: true, result });
  } catch (e: any) {
    (self as any).postMessage({ id, ok: false, error: e?.message ?? String(e) });
  }
};
