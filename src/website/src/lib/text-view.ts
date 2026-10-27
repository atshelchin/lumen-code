// Received-text helpers for the reader view: JSON detection, formatting with syntax highlight,
// links in plain text, and safe file names.

// JSON is shown formatted only when the whole text is an object or an array.
export function isJson(text: string): boolean {
	const s = text.trim();
	const a = s[0];
	const z = s[s.length - 1];
	if (!((a === '{' && z === '}') || (a === '[' && z === ']'))) return false;
	try {
		JSON.parse(s);
		return true;
	} catch {
		return false;
	}
}

// String with the unrolled-loop form: no catastrophic backtracking on long strings.
const TOKEN =
	/\s*("[^"\\]*(?:\\.[^"\\]*)*"|-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?|true|false|null|[{}[\],:])/y;
const ESC: Record<string, string> = { '&': '&amp;', '<': '&lt;', '>': '&gt;' };
const esc = (s: string) => s.replace(/[&<>]/g, (c) => ESC[c]);

// Re-indents valid JSON token by token, so number and string literals stay exactly as received
// (JSON.parse + JSON.stringify would round large integers and drop duplicate keys).
// Returns one entry per line: HTML with <span class="j-*"> when html is true, else plain text.
//   j-k key, j-s string, j-n number, j-b true/false, j-z null, j-p punctuation
export function formatJson(text: string, { html = false, indent = 2 } = {}): string[] {
	const toks: string[] = [];
	TOKEN.lastIndex = 0;
	for (let m; (m = TOKEN.exec(text));) toks.push(m[1]);
	const pad = (d: number) => ' '.repeat(d * indent);
	const put = html
		? (cls: string, s: string) => `<span class="j-${cls}">${esc(s)}</span>`
		: (_cls: string, s: string) => s;
	const lines: string[] = [];
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

// Plain text cut at its links, for rendering: [text, link, text, link, …, text].
export function linkParts(text: string): { text: string; href?: string }[] {
	const out: { text: string; href?: string }[] = [];
	let last = 0;
	for (const m of text.matchAll(URL_RE)) {
		out.push({ text: text.slice(last, m.index) });
		out.push({ text: m[0], href: m[0] });
		last = (m.index ?? 0) + m[0].length;
	}
	out.push({ text: text.slice(last) });
	return out;
}

// A received name comes from the sender (PROTOCOL §13): never a path, and without control or
// bidirectional-override characters (which can disguise an extension).
export function safeName(name: string | null | undefined): string {
	const clean = (name || '')
		// eslint-disable-next-line no-control-regex -- removing control characters is the point
		.replace(/[\u0000-\u001f\u007f-\u009f\u061c\u200e\u200f\u202a-\u202e\u2066-\u2069]/g, '')
		.replace(/[\\/:*?"<>|]/g, '_')
		.trim();
	return clean === '' || clean === '.' || clean === '..' ? 'file' : clean;
}
