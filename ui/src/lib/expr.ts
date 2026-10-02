// 表示用の小さな式評価（図形エディタで、描いている途中の形をすぐに見せるため）。
// 正式な計算は Rust の式エンジン（formdoc-expr）が行う。ここでは同じ書き方（+ - * / ^ **、括弧、関数）を読むだけ。

const FUNCS: Record<string, (...a: number[]) => number> = {
  sqrt: Math.sqrt,
  root: (x, n) => Math.pow(x, 1 / n),
  abs: Math.abs,
  min: Math.min,
  max: Math.max,
  sin: Math.sin,
  cos: Math.cos,
  tan: Math.tan,
  log: Math.log10,
  ln: Math.log,
  exp: Math.exp,
  round: (x, d = 0) => Math.round(x * 10 ** d) / 10 ** d,
  floor: Math.floor,
  ceil: Math.ceil,
};

type Tok = { t: 'num' | 'id' | 'op'; v: string };

function lex(s: string): Tok[] | null {
  const out: Tok[] = [];
  let i = 0;
  while (i < s.length) {
    const c = s[i];
    if (/\s/.test(c)) i++;
    else if (/[0-9.]/.test(c)) {
      const m = /^(\d+\.?\d*|\.\d+)(e[+-]?\d+)?/i.exec(s.slice(i));
      if (!m) return null;
      out.push({ t: 'num', v: m[0] });
      i += m[0].length;
    } else if (/[A-Za-z]/.test(c)) {
      const m = /^[A-Za-z][A-Za-z0-9_]*/.exec(s.slice(i))!;
      out.push({ t: 'id', v: m[0] });
      i += m[0].length;
    } else if (s.startsWith('**', i)) {
      out.push({ t: 'op', v: '^' });
      i += 2;
    } else if ('+-*/^(),×÷'.includes(c)) {
      out.push({ t: 'op', v: c === '×' ? '*' : c === '÷' ? '/' : c });
      i++;
    } else return null;
  }
  return out;
}

/** 式を評価する。読めない・未定義の変数があるときは null */
export function evalExpr(src: string, scope: (name: string) => number | undefined): number | null {
  const lexed = lex(src);
  if (!lexed || !lexed.length) return null;
  const toks: Tok[] = lexed;
  let p = 0;
  const peek = () => toks[p];
  const fail = () => {
    throw new Error();
  };
  const expect = (v: string) => (peek()?.v === v ? p++ : fail());

  function primary(): number {
    const t = toks[p++] ?? fail();
    if (t!.t === 'num') return Number(t!.v);
    if (t!.t === 'id') {
      if (peek()?.v === '(') {
        p++;
        const args: number[] = [];
        if (peek()?.v !== ')') {
          args.push(add());
          while (peek()?.v === ',') {
            p++;
            args.push(add());
          }
        }
        expect(')');
        const f = FUNCS[t!.v] ?? fail();
        return (f as (...a: number[]) => number)(...args);
      }
      if (t!.v === 'pi') return Math.PI;
      const v = scope(t!.v);
      return v === undefined ? fail() : v;
    }
    if (t!.v === '(') {
      const v = add();
      expect(')');
      return v;
    }
    return fail();
  }
  function unary(): number {
    if (peek()?.v === '-') {
      p++;
      return -unary();
    }
    if (peek()?.v === '+') {
      p++;
      return unary();
    }
    return pow();
  }
  function pow(): number {
    const b = primary();
    if (peek()?.v === '^') {
      p++;
      return Math.pow(b, unary()); // 右結合
    }
    return b;
  }
  function mul(): number {
    let v = unary();
    while (peek()?.v === '*' || peek()?.v === '/') v = toks[p++].v === '*' ? v * unary() : v / unary();
    return v;
  }
  function add(): number {
    let v = mul();
    while (peek()?.v === '+' || peek()?.v === '-') v = toks[p++].v === '+' ? v + mul() : v - mul();
    return v;
  }
  try {
    const v = add();
    if (p !== toks.length || !Number.isFinite(v)) return null;
    return v;
  } catch {
    return null;
  }
}

/** 図形の文字の {{変数}}・{{式}}・{{式:桁}} を値にする（Rust の shape_label と同じ規則） */
export function formatLabel(
  label: string,
  scope: (name: string) => number | undefined,
  varText: (name: string) => string | undefined,
): string {
  return label.replace(/\{\{\s*([^{}]+?)\s*\}\}/g, (all, inner: string) => {
    const m = /^(.*?):\s*(\d+)\s*$/.exec(inner);
    const expr = (m ? m[1] : inner).trim();
    if (!m && /^[A-Za-z][A-Za-z0-9_]*$/.test(expr) && varText(expr) !== undefined) return varText(expr)!;
    const v = evalExpr(expr, scope);
    if (v == null) return all;
    const d = m ? Number(m[2]) : 3;
    return v.toLocaleString('en-US', { minimumFractionDigits: d, maximumFractionDigits: d, useGrouping: Math.abs(v) >= 10000 });
  });
}
