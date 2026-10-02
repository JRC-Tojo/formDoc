// 変数パレットからの差し込み先（最後にフォーカスした入力欄）を覚えておく。
// 本文系の欄には {{名前}}、式の欄には 名前 を差し込む。

let target: HTMLInputElement | HTMLTextAreaElement | null = null;

export function track(el: HTMLInputElement | HTMLTextAreaElement) {
  const onFocus = () => (target = el);
  el.addEventListener('focus', onFocus);
  return {
    destroy() {
      el.removeEventListener('focus', onFocus);
      if (target === el) target = null;
    },
  };
}

export function insertVar(name: string): boolean {
  const el = target;
  if (!el || !document.body.contains(el)) return false;
  const text = el.dataset.insert === 'name' ? name : `{{${name}}}`;
  const start = el.selectionStart ?? el.value.length;
  const end = el.selectionEnd ?? start;
  el.value = el.value.slice(0, start) + text + el.value.slice(end);
  el.selectionStart = el.selectionEnd = start + text.length;
  el.dispatchEvent(new Event('input', { bubbles: true }));
  el.focus();
  return true;
}
