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
