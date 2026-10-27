# Lumen Code（光码）

从屏幕到相机的高速文件传输。发送端把文件播放成彩色格子组成的动画，接收端用相机拍下来并还原文件。不需要网络，不需要配对，也没有回传通道。

[English](README.md) · [开发者指南](docs/guide/README.zh-CN.md) · [协议规范（中文）](docs/PROTOCOL.zh-CN.md) · [Protocol (English, STE)](docs/PROTOCOL.md) · [测试向量](test/vectors/test-vectors.json) · [Rust 库](src/rust/README.md)

## 为什么不直接用动态二维码？

动态二维码容易做，但慢：一帧 QR v25 只能装约 1.2 KB，而且方形用不满手机和显示器的大部分屏幕。Lumen Code 的用法一样，相机对准屏幕即可，但每帧装的数据更多：

| | 动态二维码（v25，常见） | Lumen Code v1 |
|---|---|---|
| 每格比特 | 1 | 2（C4，默认）或 3（C8） |
| 形状 | 正方形 | 铺满屏幕（自适应宽高，宽高比最高 2.4） |
| 格子大小 | 由版本决定 | 按屏幕物理尺寸自动选择 |
| 每帧数据（手机屏） | 约 1.2 KB | 约 2.2 KB（C4）/ 3.4 KB（C8） |
| 每帧数据（13 寸笔记本） | 约 1.2 KB | 约 6.9 KB（C4）/ 10.3 KB（C8） |
| 丢帧 | 通常循环重放 | 喷泉码：每帧都是新数据 |
| 一个角被反光挡住 | 整帧失败 | 用其余三个角继续解码 |

默认每秒 12 帧时，笔记本屏幕的原始容量约 80–125 KB/s。实际能收到多快取决于接收端的相机，接收页面会实时显示。速度来自哪些环节，[性能指南](docs/guide/04-performance.zh-CN.md)里有逐项的数字。

## 试用

在线地址：<https://lumencode.awesometools.dev/zh>。在发送端屏幕打开「发送」并选择文件，在接收端手机打开「接收」。两个页面在手机和电脑上都能用，无需安装，文件只在两台设备之间传递。

> **光敏提示：** 发送时，码会以每秒最多 60 次的频率闪烁变色。对闪光敏感的人请不要注视。网页和原生 App 都会在码开始播放前显示这条提示。

- **发送端**：发送文件，或直接输入、粘贴文本（文件 | 文本 两个标签）。可调密度（低 / 自动 / 高 / 极高）、4 色或 8 色、纠错等级、帧率、引擎。轻点画面显示或隐藏控制栏。
- **接收端**：文本和图片在全屏阅读视图中打开（JSON 自动格式化并高亮，链接可点击，字号可调）；文件可保存或打开。扫描时显示进度、速度、格子在相机中的像素大小、成功率，以及一行诊断信息（引擎耗时、相机帧率、识别次数、新帧数、发送帧率、捕获率）和提示。传输完成后自动关闭相机，本次传输的各项数据仍可查看。相机支持变焦时，可以双指在画面上缩放。每个数字的含义见[仪表解读](docs/guide/05-dashboard.zh-CN.md)。
- **引擎切换**：两个页面都可以在 JavaScript 实现和编译成 WebAssembly 的 Rust 实现之间切换（`JS | WASM`），两者生成的帧逐位一致。接收端默认用 WASM，扫描中途切换不会丢失进度。
- **适配屏幕**：码按屏幕自适应宽高，横屏竖屏都能铺满。码是横的时，把接收端手机横过来即可。

自己运行网页应用（SvelteKit，位于 [`src/website`](src/website/README.md)）：

```sh
cd src/website && bun install
bun run dev            # http://localhost:5173（localhost 下可用相机）
bun run dev:https      # https://<局域网IP>:5173，供手机使用（自签名证书）
bun run build          # 预渲染页面和 Cloudflare Worker，输出到 .svelte-kit/cloudflare
```

## 原生应用（iOS、Android、桌面）

原生应用做的是同样的传输，三个平台共用一套 Rust 核心（[`src/app`](src/app/README.md)：应用逻辑、扫描流水线和协议库）。各平台外壳只负责绘制界面和调用系统接口。外观和操作与网页一致，另外用上了原生才有的能力：原地读取相机图像数据、相机支持时以 60–240 帧采集、Android 上由应用自己控制曝光、120 Hz 播放、震动反馈、调节屏幕亮度，以及手机发热时减少计算量。

| 应用 | 外壳 | 构建并运行 | 说明 |
|---|---|---|---|
| iOS 17+ | SwiftUI | `src/ios/scripts/run.sh` | [src/ios](src/ios/README.md) |
| Android 10+ | Jetpack Compose | `cd src/android && ./gradlew :app:installDebug` | [src/android](src/android/README.md) |
| macOS、Windows、Linux | GPUI | `cd src/desktop && cargo run --release` | [src/desktop](src/desktop/README.md) |

各子目录的说明文档只有英文版。

桌面版安装包在 [Releases](https://github.com/atshelchin/lumen-code/releases) 页面：已签名并经过公证的 macOS 镜像，以及 Windows、Linux 压缩包。桌面版在 macOS 上测试过；Windows、Linux 版（含摄像头接收）是新加的，还没有在真机上试过。

iPhone 和 Android 应用即将上架 App Store 和 Google Play，定价 US$2.99，一次买断（不订阅、不内购、无广告）。网页版和桌面版继续免费，任何人也都可以从本仓库免费编译所有应用。

## 在自己的项目里使用

不需要安装任何包，直接从本仓库取代码，并固定到某个标签或提交。下面的步骤编程 agent 也可以照着做。

| 你要做的 | 取什么 | 再读 |
|---|---|---|
| 网页或 Node 程序 | `src/core/` 整个目录：ES 模块，无需构建，无依赖。要用更快的引擎，再加上 `dist/wasm/lumen.wasm`，由 `src/core/wasm-engine.js` 加载 | [集成指南](docs/guide/03-integration.zh-CN.md) §3.2–§3.3 |
| Rust 程序 | 在 `Cargo.toml` 里写 `lumen-code = { git = "https://github.com/atshelchin/lumen-code" }`；库名为 `lumen`（无依赖） | [src/rust/README.md](src/rust/README.md) |
| 其他语言的应用（Swift、Kotlin、C……） | 同一个 crate 的 C ABI：`src/rust/include/lumen.h`，用 `--features ffi` 构建 | [集成指南](docs/guide/03-integration.zh-CN.md) §3.5 |
| 全新的实现 | [规范](docs/PROTOCOL.zh-CN.md)和 [`test/vectors/test-vectors.json`](test/vectors/test-vectors.json) | [集成指南](docs/guide/03-integration.zh-CN.md) §3.7 |

所有集成都必须遵守以下规则（[规范](docs/PROTOCOL.zh-CN.md) §10.4、§11、§13）：

- 直接用现成的接收端，不要改动。它会执行各项上限，并在文件校验失败时重新开始传输。
- 文件名和内容都来自发送端。不要把文件名当作路径使用，文件本身按来源不明的文件对待。
- 码开始播放前要提醒用户，因为它每秒会闪烁多次。

## 原理简述

1. 文件可选用 DEFLATE 压缩，再切分成源块。
2. 类 Raptor 喷泉码把每个源块变成无限长的符号流。一个 K 个符号的源块，收到约 K + 2 个不同的符号即可恢复（实测平均比 K 多约 1.6 个）。
3. 每个符号加上 CRC 成为数据包，再编码为 Reed–Solomon 码字。
4. 码字沿 Hilbert 曲线排进网格，每个码字占一块紧凑的区域，相机拍到撕裂帧时只丢少量码字。
5. 三个定位图案、一个角标、一组校准点阵和四份帧头，让接收端在透视、镜头畸变、模糊和摩尔纹下仍能定位网格。

[开发者指南](docs/guide/README.zh-CN.md)配有图和具体数字；完整的规范性描述见 [docs/PROTOCOL.zh-CN.md](docs/PROTOCOL.zh-CN.md)。

## 目录

```
docs/PROTOCOL.md, *.zh-CN.md     协议规范（英文 STE / 中文）
docs/guide/                      开发者指南（原理、识别、接入、性能、仪表解读）
src/core/                        JavaScript 参考实现（协议库，ES 模块）
src/website/                     网页应用（SvelteKit）：首页、发送端、接收端，中英文
src/rust/                        Rust 参考实现（零依赖，C ABI，WASM）
src/app/                         原生应用共用的核心（应用逻辑、扫描流水线、uniffi 绑定）
src/ios/ src/android/ src/desktop/   原生外壳（SwiftUI、Jetpack Compose、GPUI）
src/web/、dist/                  旧版静态站点（切换到 src/website 之前仍在线上）；dist/wasm/lumen.wasm 是 WASM 引擎
src/tools/                       WASM 构建、旧站点的构建与开发服务器、配图与分享卡片生成
test/                            Node 测试、相机模拟器、鲁棒性测试
test/vectors/test-vectors.json   一致性测试向量（由 test/vectors.js 生成）
```

## 测试

```sh
npm test                 # 一致性向量、模糊/属性测试、核心、数据包/源块、经相机模型的端到端、JS<->WASM 互通
npm run test:fuzz        # 模糊/属性测试，用例数 ×20
npm run test:robustness  # 透视、旋转、镜像、模糊、光晕、摩尔纹、运动模糊、曝光
cd src/rust && cargo test --release   # 同一套向量、与 JS 的差分测试、模糊/属性测试、解码 JS 模拟的相机帧
cd src/app && cargo test --release    # 原生应用核心：应用逻辑、相机流水线、卷帘快门模拟
node test/bench.js; cd src/rust && cargo run --release --example measure -- bench   # 各环节吞吐
cd src/website && bun run test:e2e    # 网页应用在 Chromium 中：录好的传输视频当摄像头，两种引擎、两种语言
```

相机模拟器（`test/camera.js`）模拟透视、桶形畸变、光学模糊、颜色串扰、白平衡、暗角、光晕、摩尔纹、运动模糊、4:2:0 色度下采样和噪声。在随机的"手持、没对准"测试中，4 色模式下 30 帧约解出 27–28 帧，码字成功率 86–89%。解不出的帧都是每格只有约 3 个相机像素、又严重模糊的情况。

## 性能（Apple M2，1080p 相机帧）

| 识别引擎 | 手机尺寸的码 | 笔记本尺寸的码 |
|---|---|---|
| JavaScript（V8） | 14–16 ms | 41–45 ms |
| Rust → WebAssembly | 9–10 ms | 21 ms |
| Rust 原生 | 7–9 ms | 16 ms |

此外，接收端会跳过已经解出的帧（只读帧头，JS 约 8 ms）；识别频率跟随发送端帧率，每个发送帧约识别 1.6 次，而不是相机每帧都识别；交给识别线程之前，先由 GPU 只截取码所在的区域并缩放到每格约 6 像素。视频上方的面板不用背景模糊，传输完成后自动关闭相机。这些措施合起来让计算量降到几分之一，手机不容易发热。

## 状态

- 协议 1.0 版为正式版，已冻结（2026-10-20）：线格式不会再改，已发布的桌面 App v0.1.x 用的就是它。规范覆盖线格式和接收端：合法性规则、传输状态机、资源上限、错误、安全考虑和扩展规则。
- 两个独立实现（JavaScript、Rust）通过同一套一致性向量：编码、合法与非法输入的解码、经过接收端状态机的包流。在伪随机语料（2 万个帧头、2 万个描述符、5000 个损坏的 DEFLATE 流、2000 组喷泉符号）上，两者的判定完全一致。
- 目前的验证：模拟器、Chrome 虚拟摄像头，以及初步的真机测试。真机调优仍在进行，欢迎反馈效果不好的机型和环境。
- 反馈问题时请附上版本号。网页在首页底部以及发送、接收卡片底部显示（如"版本 9cc2078 · 2026-10-20"，即源码的提交 ID）；App 在首页底部和接收完成后的详情里显示。
- 许可证：[MIT](LICENSE)。
- 网站在 `src/website/src/app.html` 里加载一个访问统计脚本（`tj.appsdata.org`，见[隐私政策](https://lumencode.awesometools.dev/zh/privacy)）。自行部署时把它删掉，并删去 `src/website/vite.config.ts` 中 CSP 里的对应地址。
