# 3 · 接入指南

[指南目录](README.zh-CN.md) · [English](03-integration.md) · 规范对应章节：§10、§11、§15

## 3.1 选哪个实现

| 场景 | 推荐 | 位置 |
|---|---|---|
| 网页，想最快跑起来 | JavaScript（ES 模块，零依赖） | `src/core/` |
| 网页，接收端要省电、要快 | Rust 编译的 WebAssembly（约 220 KB，无导入） | `dist/wasm/lumen.wasm` + `src/core/wasm-engine.js` |
| Rust 程序 | `lumen-code` crate（零依赖，MSRV 1.73） | `src/rust/` |
| Android / iOS / 桌面原生应用 | 同一个 crate 的 C ABI | `src/rust/include/lumen.h` |

所有实现生成的帧**逐位相同**，可以混用：JS 发送、Android 接收，或者 Rust 发送、浏览器接收。

## 3.2 网页：JavaScript

### 发送

```js
import { Sender, preparePayload } from './core/transfer.js';
import { Renderer } from './core/render.js';

const prepared = await preparePayload(fileBytes, 'photo.jpg'); // 能压缩就 DEFLATE 压缩
const tx = new Sender(prepared, { mode: 1 /* C4 */, ecc: 1 /* 16% */, X: 90, Y: 160 });

const canvas = document.querySelector('canvas');
const ctx = canvas.getContext('2d');
let r, img;
setInterval(() => {
  const { cells, layout } = tx.nextFrame();
  if (!r) {
    r = new Renderer(layout, 1, 6); // 每格 6 个设备像素，四周 2 格静区
    canvas.width = r.width;
    canvas.height = r.height;
    img = ctx.createImageData(r.width, r.height);
  }
  r.draw(cells, new Uint32Array(img.data.buffer));
  ctx.putImageData(img, 0, 0);
}, 1000 / 12); // 每秒 12 帧
```

画布要按**设备像素**显示（CSS 尺寸 = 像素尺寸 / devicePixelRatio），否则浏览器缩放会把格子边缘糊掉。X、Y 和格子大小的选法见 §3.6。

### 接收

```js
import { Scanner } from './core/scan.js';
import { Receiver } from './core/transfer.js';

const video = document.querySelector('video');
video.srcObject = await navigator.mediaDevices.getUserMedia({
  video: { facingMode: 'environment', width: { ideal: 1920 }, height: { ideal: 1080 } },
});
await video.play();

const canvas = new OffscreenCanvas(video.videoWidth, video.videoHeight);
const g = canvas.getContext('2d', { willReadFrequently: true });
const scanner = new Scanner();
const rx = new Receiver(); // 参考上限；要修改上限，用 new Receiver({ maxSize, maxKept, maxTransfers })

async function tick() {
  g.drawImage(video, 0, 0);
  const res = scanner.scan(g.getImageData(0, 0, canvas.width, canvas.height));
  if (res.ok) for (const cw of res.codewords) if (cw.ok) await rx.push(cw.bytes);
  const out = await rx.tryFinish(); // { name, bytes } | { error: 'checksum' }（传输重新开始） | null
  if (out && out.bytes) return save(out.name, out.bytes);
  setTimeout(tick, 50);
}
tick();
```

这是最小写法。正式使用时应把识别放进 Web Worker，并按 [指南 2 §2.4](02-scanner.zh-CN.md) 的方式调度；`src/website/src/lib/recv/scan.worker.ts`、`sink.worker.ts` 和 `receiver.svelte.ts` 是完整的参考。

接收端实现的是 PROTOCOL §11 的状态机：

- `rx.push(packet)` 返回它对这个数据包做了什么：`'discard'`（丢弃）、`'keep'`（暂存，等文件描述符到达）、`'add'`（加入解码器）、`'accept'`（接受了一个描述符）或 `'refuse'`（描述符所描述的传输超过上限，拒收）。
- `rx.progress()` 返回最新一个数据包所属传输的 `{ state, name, size, received, needed, done, T }`。`state` 为 `'collecting'`（还没有描述符）、`'receiving'`、`'refused'`（要告诉用户文件太大）或 `'complete'`。
- `rx.tryFinish()` 只返回一次文件。如果解出的文件没有通过校验（大小或 CRC-32 不对，极少发生），它返回 `{ error: 'checksum' }` 并自动重新开始这次传输，继续扫描即可。
- 上限（§11.4）：默认每个文件 256 MiB，描述符到达前最多暂存 4096 个数据包，同时最多 4 个传输。文件名来自发送端，绝不能当作路径使用（§13）。

## 3.3 网页：WebAssembly

`wasm-engine.js` 把 WASM 模块包装成与 JS 版**同样形状**的类，改一行 import 就能切换：

```js
import { loadWasm, WasmScanner, WasmSender, WasmReceiver } from './core/wasm-engine.js';

await loadWasm('./wasm/lumen.wasm'); // 也可以传入 ArrayBuffer（例如在 Node 里）
const scanner = new WasmScanner();   // scan() 的返回值与 Scanner 相同
const rx = new WasmReceiver();       // push() / progress() / tryFinish() 与 Receiver 相同；用完后调用 free()
const tx = new WasmSender(prepared, { mode: 1, ecc: 1, X: 90, Y: 160 });
// 不再使用时：scanner.free()、tx.free() 释放 WASM 内存
```

WASM 识别比 JS 快 1.5–2 倍，手机发热明显降低。模块用 `npm run build:wasm` 生成（需要 Rust 工具链和 `wasm32-unknown-unknown` 目标）。

## 3.4 Rust

直接从本仓库引入 crate（它没有任何依赖；可用 `tag = "…"` 或 `rev = "…"` 固定版本）：

```toml
[dependencies]
lumen-code = { git = "https://github.com/mondaylabsltd/lumen-code" }
```

库名为 `lumen`：

```rust
use lumen::scan::{Image, Scanner};
use lumen::transfer::{prepare, Finish, Receiver, Sender};
use lumen::Renderer;

// 发送
let prepared = prepare(&file_bytes, "photo.jpg", true);
let mut tx = Sender::new(&prepared, 1 /* C4 */, 1 /* ecc */, 90, 160, 0x1234 /* tid */, 0 /* seq */).unwrap();
let frame = tx.next_frame();
let r = Renderer::new(&tx.layout, tx.mode, 6 /* px/格 */, 2 /* 静区 */);
let mut pixels = vec![0u32; r.width * r.height];
r.draw(&frame.cells, &mut pixels);

// 接收
let mut scanner = Scanner::new();
let mut rx = Receiver::new();
let res = scanner.scan(&Image { width, height, rgba: &camera_rgba }, None);
for cw in res.codewords.iter().filter(|c| c.ok) {
    rx.push(cw.bytes.as_ref().unwrap());
}
if let Some(Finish::File { name, bytes }) = rx.try_finish() { /* 完成 */ }
```

TID 应当随机生成（每次传输不同），seq 初值也建议随机。

## 3.5 原生应用：C ABI

```sh
cd src/rust
cargo build --release --features ffi                                   # 桌面：liblumen.{so,dylib,dll}
cargo build --release --features ffi --target aarch64-apple-ios        # iOS：liblumen.a
cargo ndk -t arm64-v8a build --release --features ffi                  # Android：liblumen.so
```

Android 通过 JNI 调用，iOS 通过桥接头文件调用。所有句柄都是不透明指针，库返回的内存在同一句柄的下一次调用前有效。

### 发送

```c
#include "lumen.h"

WSender *tx = lm_sender_new_file(file, file_len, (const uint8_t *)"photo.jpg", 9,
                                 1 /* 压缩 */, 1 /* C4 */, 1 /* ecc */, 90, 160,
                                 random_u16(), random_u32());
const uint32_t *info = lm_sender_info(tx); // n, nsym, T, perFrame, K, X, Y, mode, ecc, blocks
for (;;) {                                  // 每 1/12 秒一次
    const uint8_t *cells = lm_sender_next_frame(tx); // X*Y 个格子值
    draw_cells(cells, info[5], info[6]);
}
lm_sender_free(tx);
```

格子值：0–7 是当前颜色模式调色板的索引，8 是墨色 (10, 13, 20)，9 是纸色 (242, 244, 248)。调色板见规范 §5。按 §3.6 的规则画格子；`src/core/render.js` 是一个圆角"LED"风格的参考画法。

### 接收

```c
WScanner *sc = lm_scanner_new();
WReceiver *rx = lm_receiver_new();

// 每个相机帧（RGBA，宽 w、高 h）：
const uint8_t *rec = lm_scan(sc, rgba, w, h, NULL, 0);
// 解析结果记录（小端）：
//   u32 ok, skipped, reason, candidates
//   u32 mode, X, Y, n, nsym, seq
//   f64 cellPx, corners[8], markers
//   u32 cornerFound, cwOk, cwTotal, cwCount, corrected
//   u32 nPackets, packetLen
//   nPackets × { u32 码字序号, packetLen 字节的数据包 }
for (each packet p) lm_receiver_push(rx, p, packetLen);  // 返回 0 丢弃，1 暂存，2 加入，3 接受，4 拒收

// lm_receiver_progress(rx)：u32[10] valid, tid, received, needed, blocks, done, T, fileSize,
//                           state（0 collecting，1 receiving，2 refused，3 complete）, payloadLength
if (lm_receiver_finish(rx) == 1) {          // 0 未完成，1 完成，2 校验失败：传输重新开始
    const uint8_t *name = lm_receiver_name(rx);  uint32_t nl = lm_receiver_name_len(rx);
    const uint8_t *data = lm_receiver_file(rx);  uint32_t dl = lm_receiver_file_len(rx);
}
```

seq 为 0xFFFFFFFF 表示帧序号未知：所有帧头副本都读不出，识别器沿用了上一个正确的帧头（数据照常可用，但不能用来跳帧）。`src/core/wasm-engine.js` 里的 `WasmScanner.scan()` 是解析这个记录的完整参考，因为 WASM 和原生用的是同一套 C ABI。`skip` 参数用于跳过已解出的帧，格式见 `lumen.h`。

## 3.6 参数怎么选

| 参数 | 建议 | 说明 |
|---|---|---|
| 网格形状 | 铺满可用的屏幕区域，宽高比 ≤ 2.4 | 格子越多，每帧数据越多；码是横的，接收端把手机横过来即可 |
| 格子物理大小 | 约 0.75 mm | 短边 52–150 格。演示页面的"密度"在此基础上乘 0.72 / 1 / 1.3 / 1.65 |
| 每格像素 | 整数个设备像素，≥ 3 | 非整数会让格子宽窄不一 |
| 静区 | 四周 ≥ 2 格墨色 | 让定位图案和背景分开 |
| 颜色模式 | C4 | C8 多 50% 数据，但需要良好对焦、较少摩尔纹 |
| 纠错 | 16%（中） | 环境差用 22%；10% 只适合很稳定的固定装置 |
| 帧率 | 12 帧/秒 | 相机 30 帧/秒时，每个发送帧大约被拍到 2.5 次。接收端报告有余量时再提高 |

传输过程中可以随时改变网格尺寸、颜色模式和纠错等级（例如窗口大小变化时）。只要 T（每个符号的字节数）不变，接收端就不受影响，接着收。改变 T 必须用新的 TID 开始一次新传输。

## 3.7 做一个新的兼容实现

1. 通过 `test/vectors/test-vectors.json` 中的全部向量（PROTOCOL §15）。`test/test-vectors.js` 和 `src/rust/tests/vectors.rs` 展示了每组向量的用法：
   - **编码**：CRC、mix32、Rng、Reed–Solomon 码字、网格布局、帧头比特、由给定数据包生成的帧、数据包、描述符、喷泉码参数与符号、源块划分；
   - **解码**：必须接受或必须丢弃的帧头、数据包和描述符（每个应丢弃的向量恰好有一处缺陷，并注明了缺陷名称），纠错能力范围内的受损 Reed–Solomon 码字，可解或不可解的喷泉符号集合；
   - **数据流**（`streams`）：送入一个全新接收端的数据包序列，附带每个数据包对应的 §11.2 动作和每次解码尝试的结果，覆盖状态机、文件校验及其重新开始、各项上限、DEFLATE 边界情况；
2. 用你的发送端生成帧，交给参考接收端解码（或者反过来）；
3. 用 `node test/make-raw.js` 生成经过相机模拟的帧，让你的识别器去解；
4. 可选：复现 `test/corpus.js` 中的伪随机语料，把你的各项判定的摘要与向量中 `differential` 一节对比。这样能在数千个边界情况上与参考解码器逐一对比。

发包调度、参数选择、识别算法和显示方式由你决定（§10.2–§10.4、§11.6），不影响兼容性。如果想生成与参考发送端完全相同的帧，`referenceSender` 向量给出了参考做法。
