// Reader-view helpers of the demo receiver: JSON detection, formatting, link detection.
import { isJson, formatJson, linkify, safeName } from '../src/web/js/text-view.js';

let failures = 0;
function check(cond, msg) {
  if (!cond) {
    failures++;
    console.log('FAIL', msg);
  }
}

// received file names are not paths and show no hidden characters (PROTOCOL §13)
check(safeName('../../etc/passwd') === '.._.._etc_passwd' && safeName('a\\b:c') === 'a_b_c', 'separators replaced');
check(safeName('..') === 'file' && safeName('.') === 'file' && safeName('') === 'file' && safeName(null) === 'file', 'empty and dot names');
check(safeName('invoice\u202Efdp.exe') === 'invoicefdp.exe' && safeName('a\u0000b\nc') === 'abc', 'control and bidi characters removed');
check(safeName('报告 2026.pdf') === '报告 2026.pdf', 'ordinary names unchanged');

check(isJson('{"a":1}') && isJson(' [1, 2] \n'), 'objects and arrays are JSON');
check(!isJson('123') && !isJson('"text"') && !isJson('{bad}') && !isJson('') && !isJson('[1,'), 'non-JSON rejected');

// literals stay exactly as received (JSON.parse would round the big integer)
const src = '{"id":12345678901234567890,"s":"a \\"q\\" <b> 中","e":[],"o":{},"n":[1,-2.5e3,true,false,null,{"x":"y"}]}';
const lines = formatJson(src);
const pretty = lines.join('\n');
check(pretty.includes('"id": 12345678901234567890'), 'big integer kept verbatim');
check(pretty.includes('"e": [],') && pretty.includes('"o": {},'), 'empty containers on one line');
check(JSON.stringify(JSON.parse(pretty)) === JSON.stringify(JSON.parse(src)), 'formatted JSON parses to the same value');
check(lines[2] === '  "s": "a \\"q\\" <b> 中",', 'escapes untouched, two-space indent');
const html = formatJson(src, { html: true });
check(html.length === lines.length, 'html and text give the same lines');
check(html[2].includes('<span class="j-k">"s"</span>') && html[2].includes('&lt;b&gt;'), 'keys classed, HTML escaped');
check(html.some((l) => l.includes('<span class="j-n">-2.5e3</span>')) && html.some((l) => l.includes('<span class="j-z">null</span>')), 'number and null classes');

// a long string must not blow the regex up
const long = JSON.stringify({ data: 'x'.repeat(2_000_000), list: Array.from({ length: 50_000 }, (_, i) => i) });
const t0 = performance.now();
check(formatJson(long).length === 50_005, 'large JSON formats');
const ms = performance.now() - t0;
check(ms < 2000, `large JSON formats quickly (${ms.toFixed(0)} ms)`);

// links: ASCII only, trailing punctuation outside
globalThis.document = { createElement: () => ({}) };
const parts = [];
linkify({ append: (x) => parts.push(typeof x === 'string' ? x : `[${x.href}]`) }, '见 https://a.b/c，然后 (https://x.y/p?q=1&r=2#f) 或 https://g.co/。完');
check(parts.join('') === '见 [https://a.b/c]，然后 ([https://x.y/p?q=1&r=2#f]) 或 [https://g.co/]。完', `linkify: ${parts.join('')}`);

if (failures) {
  console.log(`${failures} failures`);
  process.exit(1);
}
console.log('view: all passed');
