/**
 * Subsequence fuzzy match. Returns a score (higher is better) or null when
 * `query` is not a subsequence of `text`. Contiguous runs and word starts score more.
 */
export function fuzzyScore(query: string, text: string): number | null {
  const q = query.toLowerCase();
  const s = text.toLowerCase();
  if (!q) return 0;
  const direct = s.indexOf(q);
  if (direct >= 0) return 100 + (direct === 0 ? 50 : 0) - direct * 0.1;
  let score = 0;
  let run = 0;
  let si = 0;
  for (let qi = 0; qi < q.length; qi++) {
    const ch = q[qi];
    let found = -1;
    while (si < s.length) {
      if (s[si] === ch) {
        found = si;
        break;
      }
      si++;
    }
    if (found < 0) return null;
    const prev = found > 0 ? s[found - 1] : " ";
    const wordStart = /[\s\-_·.:/|(]/.test(prev);
    run = found > 0 && qi > 0 && s[found - 1] === q[qi - 1] ? run + 1 : 0;
    score += 1 + run * 2 + (wordStart ? 3 : 0);
    si = found + 1;
  }
  return score;
}

/** Every whitespace-separated token must fuzzy-match at least one field. */
export function fuzzyMatchAll(query: string, fields: string[]): number | null {
  const tokens = query.trim().split(/\s+/).filter(Boolean);
  if (tokens.length === 0) return 0;
  let total = 0;
  for (const tok of tokens) {
    let best: number | null = null;
    for (const f of fields) {
      const sc = fuzzyScore(tok, f);
      if (sc !== null && (best === null || sc > best)) best = sc;
    }
    if (best === null) return null;
    total += best;
  }
  return total;
}
