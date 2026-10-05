// 章構成の拘束（文書テンプレートの info.chapters）を GUI で使うための処理。
// 判定はすべて formdoc-core（structure.rs）が行い、評価結果の BlockResult.chapter に載せて返す。
// GUI はそれを見て操作の前に止めるだけにする（判定を1か所にするため）。

import { flatten, locate } from './tree';
import type { Block, ChapterInfo, UpdateResult } from './types';

/** 部品の章の情報（部品テンプレートの中の部品は、まとまりと同じ）。評価前・章の定義が無ければ null */
export function chapterInfo(result: UpdateResult | null, id: string | null): ChapterInfo | null {
  return (id && result?.blocks[id]?.chapter) || null;
}

/**
 * 部品 afterId の直後に置ける部品の種類（null なら制限なし）。
 * afterId が null（未選択）なら文書の末尾に置くので、最後の部品の情報を使う。
 */
export function insertableAfter(result: UpdateResult | null, blocks: Block[], afterId: string | null): string[] | null {
  const id = afterId && locate(blocks, afterId) ? afterId : (blocks[blocks.length - 1]?.id ?? null);
  return chapterInfo(result, id)?.insertable ?? null;
}

/**
 * 部品テンプレートとして保存・挿入する部品から、章の割り当て（見出しの props.chapter）を外す。
 * 部品テンプレートの中の見出しは章として数えないため。
 */
export function withoutChapters(blocks: Block[]): Block[] {
  const copy: Block[] = JSON.parse(JSON.stringify(blocks));
  for (const b of flatten(copy)) if (b.kind === 'heading') delete b.props.chapter;
  return copy;
}
