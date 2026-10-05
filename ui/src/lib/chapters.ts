// 章構成の拘束（文書テンプレートの info.chapters）を GUI で使うための処理。
// 判定の本体は formdoc-core（structure.rs）の検証。ここでは操作の前に止める（削除・移動の禁止、使える部品の絞り込み）ためだけに使う。

import { locate } from './tree';
import type { Block, Chapter, StyleInfo } from './types';

/** 構造形式を入力する文書情報の欄の key（formdoc-core の VARIANT_KEY と同じ） */
export const VARIANT_KEY = 'variant';

/** 章の定義を id で引く（入れ子も含む） */
export function chapterDef(info: StyleInfo | null, id: unknown): Chapter | null {
  if (!info || typeof id !== 'string' || !id) return null;
  const find = (cs: Chapter[]): Chapter | null => {
    for (const c of cs) {
      if (c.id === id) return c;
      const s = find(c.sections ?? []);
      if (s) return s;
    }
    return null;
  };
  return find(info.chapters ?? []);
}

/** 構造形式 variant の文書で使う章か */
export function applies(c: Chapter, variant: unknown): boolean {
  return !c.variants?.length || c.variants.includes(String(variant ?? ''));
}

/** 見出しが指す章の定義（この構造形式で使う章だけ） */
export function headingChapter(b: Block, info: StyleInfo | null, variant: unknown): Chapter | null {
  if (b.kind !== 'heading') return null;
  const c = chapterDef(info, b.props.chapter);
  return c && applies(c, variant) ? c : null;
}

/**
 * 消したり動かしたりできない章の見出しか（🔒）。
 * 拘束のある必須の章で、繰り返せない章（同じ形の章を部材ごとに置くものは、増やしたり消したりできる）。
 */
export function isLockedHeading(b: Block, info: StyleInfo | null, variant: unknown): boolean {
  const c = headingChapter(b, info, variant);
  return !!c && c.level !== 'basic' && c.required && !c.repeatable;
}

/**
 * 部品 afterId の直後に置く部品が入る章（いちばん内側の、定義のある章）。
 * 部品テンプレートのまとまりの中なら、そのまとまりの位置で判定する。章の外なら null。
 */
export function chapterAt(blocks: Block[], afterId: string | null, info: StyleInfo | null, variant: unknown): Chapter | null {
  if (!info?.chapters?.length) return null;
  const loc = afterId ? locate(blocks, afterId) : null;
  const top = loc ? (loc.ancestors[0] ?? loc.list[loc.index]) : blocks[blocks.length - 1];
  const end = top ? blocks.indexOf(top) : -1;
  const stack: { level: number; def: Chapter | null }[] = [];
  for (let i = 0; i <= end; i++) {
    const b = blocks[i];
    if (b.kind !== 'heading') continue;
    const level = Number(b.props.level ?? 2);
    while (stack.length && stack[stack.length - 1].level >= level) stack.pop();
    stack.push({ level, def: headingChapter(b, info, variant) });
  }
  for (let i = stack.length - 1; i >= 0; i--) if (stack[i].def) return stack[i].def;
  return null;
}
