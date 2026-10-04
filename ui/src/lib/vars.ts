// ブロック列の中の変数を調べる・名前を付け替える（部品テンプレートの保存と挿入で使う）。
// 変数が現れる場所は部品定義（components.toml）の項目の型で決まる：
//   var（定義）/ expr（式）/ text・multiline（本文の {{名前}}）/ varlist / table（列ごと）/ grid（セル）/ code（v-名前）

import type { Block, ComponentDef, FieldDef } from './types';

type Comps = Record<string, ComponentDef>;
export type RenameMap = Record<string, string>;

const IDENT = /[A-Za-z][A-Za-z0-9_]*/g;

/** 式中の変数名（関数呼び出し・定数 pi・数値の指数 1e5 を除く） */
function exprIdents(expr: string, fn: (name: string, start: number, end: number) => void) {
  for (const m of expr.matchAll(IDENT)) {
    const start = m.index!;
    const end = start + m[0].length;
    const prev = expr[start - 1] ?? '';
    if (/[0-9.]/.test(prev)) continue; // 1e5, 2.5e-3
    if (/^\s*\(/.test(expr.slice(end))) continue; // 関数
    if (m[0] === 'pi' || m[0] === 'ix' || m[0] === 'iy') continue; // 定数・図形の繰り返し番号
    fn(m[0], start, end);
  }
}

function renameExpr(expr: string, map: RenameMap): string {
  let out = '';
  let last = 0;
  exprIdents(expr, (name, s, e) => {
    if (map[name] === undefined) return;
    out += expr.slice(last, s) + map[name];
    last = e;
  });
  return out + expr.slice(last);
}

const REF = /\{\{\s*([^{}]+?)\s*\}\}/g;

/** {{名前}} のほか、図形の文字の {{式}}・{{式:桁}} も読む */
function splitRef(inner: string): { expr: string; digits: string } {
  const m = /^(.*?)(:\s*\d+\s*)$/.exec(inner);
  return m ? { expr: m[1].trim(), digits: m[2] } : { expr: inner.trim(), digits: '' };
}

function textRefs(text: string): string[] {
  const out: string[] = [];
  for (const m of text.matchAll(REF)) exprIdents(splitRef(m[1]).expr, (n) => out.push(n));
  return out;
}

function renameText(text: string, map: RenameMap): string {
  return text.replace(REF, (all, inner) => {
    const { expr, digits } = splitRef(inner);
    const renamed = renameExpr(expr, map);
    return renamed === expr ? all : `{{${renamed}${digits}}}`;
  });
}

const CODE_REF = /\bv-([A-Za-z][A-Za-z0-9_]*)(?![A-Za-z0-9_-])/g;

function renameCode(code: string, map: RenameMap): string {
  return code.replace(CODE_REF, (all, name) => (map[name] !== undefined ? `v-${map[name]}` : all));
}

interface Visitor {
  def(name: string): void;
  ref(name: string): void;
}

function visitValue(kind: string, f: FieldDef, value: any, v: Visitor) {
  if (value == null || value === '') return;
  switch (f.type) {
    case 'var':
      if (typeof value === 'string' && value.trim()) v.def(value.trim());
      break;
    case 'expr':
      exprIdents(String(value), (n) => v.ref(n));
      break;
    case 'text':
    case 'multiline':
      for (const n of textRefs(String(value))) v.ref(n);
      break;
    case 'varlist':
      for (const n of value as string[]) v.ref(n);
      break;
    case 'code':
      for (const m of String(value).matchAll(CODE_REF)) v.ref(m[1]);
      break;
    case 'table':
      for (const row of value as Record<string, any>[]) for (const c of f.columns ?? []) visitValue(kind, c, row[c.key], v);
      break;
    case 'grid':
      for (const row of value?.rows ?? []) for (const cell of row) for (const n of textRefs(String(cell?.text ?? ''))) v.ref(n);
      break;
  }
}

function renameValue(kind: string, f: FieldDef, value: any, map: RenameMap): any {
  if (value == null || value === '') return value;
  switch (f.type) {
    case 'var':
      return typeof value === 'string' && map[value.trim()] !== undefined ? map[value.trim()] : value;
    case 'expr':
      return renameExpr(String(value), map);
    case 'text':
    case 'multiline':
      return renameText(String(value), map);
    case 'varlist':
      return (value as string[]).map((n) => map[n] ?? n);
    case 'code':
      return renameCode(String(value), map);
    case 'table':
      return (value as Record<string, any>[]).map((row) => {
        const r = { ...row };
        for (const c of f.columns ?? []) r[c.key] = renameValue(kind, c, row[c.key], map);
        return r;
      });
    case 'grid':
      return { ...value, rows: (value?.rows ?? []).map((row: any[]) => row.map((cell) => ({ ...cell, text: renameText(String(cell?.text ?? ''), map) }))) };
    default:
      return value;
  }
}

/** 内訳と合計（sum）の内訳で名前を空にした行は「合計名_番号」になる */
function implicitDefs(b: Block): string[] {
  if (b.kind !== 'sum') return [];
  const total = String(b.props.name ?? '').trim();
  return ((b.props.items ?? []) as any[]).map((it, i) => (String(it?.name ?? '').trim() ? '' : `${total}_${i + 1}`)).filter(Boolean);
}

export interface VarUsage {
  /** 定義している変数（文書の並び順） */
  defined: string[];
  /** 定義している変数 → ブロックID */
  definedBy: Record<string, string>;
  /** 参照している変数 */
  refs: Set<string>;
  /** 定義より前で参照している（＝外から受け取る）変数 */
  external: string[];
}

export function analyze(blocks: Block[], comps: Comps): VarUsage {
  const defined: string[] = [];
  const definedBy: Record<string, string> = {};
  const refs = new Set<string>();
  const external: string[] = [];
  for (const b of blocks) {
    const defsHere: string[] = [];
    const refsHere: string[] = [];
    if (b.kind === 'group') {
      // まとまりの中の変数は外から見えない。中で足りない変数が外から受け取る変数、公開する変数が外への定義
      const inner = analyze(b.children ?? [], comps);
      refsHere.push(...inner.external);
      defsHere.push(...((b.props.exports ?? []) as string[]));
    } else {
      const fields = comps[b.kind]?.fields ?? [];
      for (const f of fields) visitValue(b.kind, f, b.props[f.key], { def: (n) => defsHere.push(n), ref: (n) => refsHere.push(n) });
      defsHere.push(...implicitDefs(b));
    }
    for (const n of refsHere) {
      refs.add(n);
      if (!(n in definedBy) && !defsHere.includes(n) && !external.includes(n)) external.push(n);
    }
    for (const n of defsHere) {
      if (!(n in definedBy)) {
        defined.push(n);
        definedBy[n] = b.id;
      }
    }
  }
  return { defined, definedBy, refs, external };
}

/** ブロック列の変数名を付け替えた複製を返す */
export function renameBlocks(blocks: Block[], comps: Comps, map: RenameMap): Block[] {
  return blocks.map((b) => {
    if (b.kind === 'group') {
      // 中の変数と同じ名前の外の変数は付け替えない（中の定義が優先）
      const inner = new Set(analyze(b.children ?? [], comps).defined);
      const sub: RenameMap = Object.fromEntries(Object.entries(map).filter(([k]) => !inner.has(k) || ((b.props.exports ?? []) as string[]).includes(k)));
      return {
        ...b,
        props: { ...b.props, exports: ((b.props.exports ?? []) as string[]).map((n) => sub[n] ?? n) },
        children: renameBlocks(b.children ?? [], comps, sub),
      };
    }
    const props = { ...b.props };
    for (const f of comps[b.kind]?.fields ?? []) if (f.key in props) props[f.key] = renameValue(b.kind, f, props[f.key], map);
    // 暗黙の内訳名（合計名_番号）は、合計名を変えると変わってしまうため明示する
    if (b.kind === 'sum') {
      const total = String(b.props.name ?? '').trim();
      props.items = ((props.items ?? []) as any[]).map((it, i) => {
        if (String(it?.name ?? '').trim()) return it;
        const implicit = `${total}_${i + 1}`;
        return map[implicit] !== undefined ? { ...it, name: map[implicit] } : it;
      });
    }
    return { ...b, props };
  });
}

/** taken と重ならない名前（L → L_2 → L_3 …） */
export function uniqueName(base: string, taken: Set<string>): string {
  if (!taken.has(base)) return base;
  for (let i = 2; ; i++) {
    const n = `${base}_${i}`;
    if (!taken.has(n)) return n;
  }
}
