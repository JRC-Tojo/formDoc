// 文書の部品の木（部品テンプレートのまとまり "group" が子要素を持つ）を扱う。
// 見出しの配下は子要素にせず、同じ並びの中で「次の同じか上の階層の見出しまで」を節とみなす。

import type { Block, VarInfo } from './types';

export interface Loc {
  /** 部品が入っている並び（文書の直下、または group の children） */
  list: Block[];
  index: number;
  /** 親の group（文書の直下なら null） */
  parent: Block | null;
  /** 文書の根から、この部品までにある group */
  ancestors: Block[];
}

export function locate(blocks: Block[], id: string, parent: Block | null = null, ancestors: Block[] = []): Loc | null {
  for (let i = 0; i < blocks.length; i++) {
    const b = blocks[i];
    if (b.id === id) return { list: blocks, index: i, parent, ancestors };
    if (b.children?.length) {
      const r = locate(b.children, id, b, [...ancestors, b]);
      if (r) return r;
    }
  }
  return null;
}

export function findBlock(blocks: Block[], id: string | null): Block | null {
  if (!id) return null;
  const l = locate(blocks, id);
  return l ? l.list[l.index] : null;
}

/** 文書の並び順（深さ優先）のすべての部品 */
export function flatten(blocks: Block[], out: Block[] = []): Block[] {
  for (const b of blocks) {
    out.push(b);
    if (b.children?.length) flatten(b.children, out);
  }
  return out;
}

/** 見出しの節の終わり（同じ並びの中で、次の同じか上の階層の見出しの位置。無ければ並びの長さ） */
export function sectionEnd(list: Block[], index: number): number {
  const b = list[index];
  if (b.kind !== 'heading') return index + 1;
  const lv = Number(b.props.level ?? 2);
  let j = index + 1;
  while (j < list.length && !(list[j].kind === 'heading' && Number(list[j].props.level ?? 2) <= lv)) j++;
  return j;
}

/** 新しい ID を振り直した複製（子要素も） */
export function cloneWithIds(b: Block, uid: () => string): Block {
  return {
    id: uid(),
    kind: b.kind,
    props: JSON.parse(JSON.stringify(b.props)),
    ...(b.children?.length ? { children: b.children.map((c) => cloneWithIds(c, uid)) } : {}),
  };
}

/** 並びの index 番目までで、開いている見出しの節（見出しのID） */
function headingStack(list: Block[], uptoInclusive: number): string[] {
  const stack: { id: string; level: number }[] = [];
  for (let i = 0; i <= uptoInclusive && i < list.length; i++) {
    const b = list[i];
    if (b.kind !== 'heading') continue;
    const lv = Number(b.props.level ?? 2);
    while (stack.length && stack[stack.length - 1].level >= lv) stack.pop();
    stack.push({ id: b.id, level: lv });
  }
  return stack.map((s) => s.id);
}

/**
 * 変数の有効範囲（見出し・group のID）のうち、ある位置を含むもの。
 * mode='at'   … その部品の位置（その部品で使える変数を調べる）
 * mode='after'… その部品の直後（そこに部品を足すときに使える変数を調べる）
 */
export function scopesAt(blocks: Block[], id: string | null, mode: 'at' | 'after'): Set<string> {
  const out = new Set<string>();
  if (!id) {
    // 文書の末尾
    for (const h of headingStack(blocks, blocks.length - 1)) out.add(h);
    return out;
  }
  const loc = locate(blocks, id);
  if (!loc) return out;
  // 祖先の group と、それぞれの並びの中で group を囲む見出し
  let list = blocks;
  for (const g of loc.ancestors) {
    const i = list.findIndex((b) => b.id === g.id);
    for (const h of headingStack(list, i)) out.add(h);
    out.add(g.id);
    list = g.children ?? [];
  }
  const upto = mode === 'after' ? loc.index : loc.index - 1;
  for (const h of headingStack(loc.list, upto)) out.add(h);
  return out;
}

/** ある位置で使える変数（定義が前にあり、有効範囲がその位置を含むもの） */
export function visibleVars(blocks: Block[], vars: VarInfo[], id: string | null, mode: 'at' | 'after'): VarInfo[] {
  const order = new Map(flatten(blocks).map((b, i) => [b.id, i]));
  const scopes = scopesAt(blocks, id, mode);
  let pos: number;
  if (!id) pos = Infinity;
  else {
    const b = findBlock(blocks, id);
    // group の直後なら、group の中身の後ろ
    const last = mode === 'after' && b?.children?.length ? flatten(b.children).at(-1)!.id : id;
    pos = (order.get(last) ?? 0) + (mode === 'after' ? 0.5 : -0.5);
  }
  return vars.filter((v) => (order.get(v.block_id) ?? Infinity) < pos && (v.scope == null || scopes.has(v.scope)));
}
