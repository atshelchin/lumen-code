// Received-text helpers for the reader view: JSON detection, formatting with syntax highlight,
// and clickable links in plain text. No DOM access at import time (also used by node tests).

// JSON is shown formatted only when the whole text is an object or an array.
export function isJson(text) {
  const s = text.trim();
  const a = s[0];
  const z = s[s.length - 1];
  if (!((a === '{' && z === '}') || (a === '[' && z === ']'))) return false;
  try {
    JSON.parse(s);
    return true;
  } catch (e) {
    return false;
  }
}

// String with the unrolled-loop form: no catastrophic backtracking on long strings.
const TOKEN = /\s*("[^"\\]*(?:\\.[^"\\]*)*"|-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?|true|false|null|[{}[\],:])/y;
const ESC = { '&': '&amp;', '<': '&lt;', '>': '&gt;' };
const esc = (s) => s.replace(/[&<>]/g, (c) => ESC[c]);

// Re-indents valid JSON token by token, so number and string literals stay exactly as received
// (JSON.parse + JSON.stringify would round large integers and drop duplicate keys).
// Returns one entry per line: HTML with <span class="j-*"> when html is true, else plain text.
//   j-k key, j-s string, j-n number, j-b true/false, j-z null, j-p punctuation
export function formatJson(text, { html = false, indent = 2 } = {}) {
  const toks = [];
  TOKEN.lastIndex = 0;
  for (let m; (m = TOKEN.exec(text)); ) toks.push(m[1]);
  const pad = (d) => ' '.repeat(d * indent);
  const put = html ? (cls, s) => `<span class="j-${cls}">${esc(s)}</span>` : (cls, s) => s;
  const lines = [];
  let cur = '';
  let depth = 0;
  for (let i = 0; i < toks.length; i++) {
    const tk = toks[i];
    const c = tk[0];
    if (c === '{' || c === '[') {
      const close = c === '{' ? '}' : ']';
      if (toks[i + 1] === close) {
        cur += put('p', tk + close);
        i++;
        continue;
      }
      cur += put('p', tk);
      lines.push(cur);
      cur = pad(++depth);
    } else if (c === '}' || c === ']') {
      lines.push(cur);
      cur = pad(--depth) + put('p', tk);
    } else if (c === ',') {
      cur += put('p', tk);
      lines.push(cur);
      cur = pad(depth);
    } else if (c === ':') {
      cur += put('p', ':') + ' ';
    } else if (c === '"') {
      cur += put(toks[i + 1] === ':' ? 'k' : 's', tk);
    } else if (c === 't' || c === 'f') {
      cur += put('b', tk);
    } else if (c === 'n') {
      cur += put('z', tk);
    } else {
      cur += put('n', tk);
    }
  }
  lines.push(cur);
  return lines;
}

// http(s) links: printable ASCII only (Chinese text often follows a link without a space);
// trailing punctuation stays outside the link.
const URL_RE = /\bhttps?:\/\/[!#-&(-;=?-_a-~]*[#-&*-+/-9=@-Z\\^-_a-z|~]/gi;

export function linkify(el, text) {
  let last = 0;
  for (const m of text.matchAll(URL_RE)) {
    el.append(text.slice(last, m.index));
    const a = document.createElement('a');
    a.href = m[0];
    a.textContent = m[0];
    a.target = '_blank';
    a.rel = 'noopener noreferrer';
    el.append(a);
    last = m.index + m[0].length;
  }
  el.append(text.slice(last));
}
