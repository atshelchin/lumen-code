# Lumen Code（光码）

用"屏幕 → 相机"高效传输文件。发送端把文件播放成一串彩色格子动画，接收端用相机拍摄并还原文件。不需要网络、不需要配对、没有回传通道。

[English](README.md) · [开发者指南](docs/guide/README.zh-CN.md) · [协议规范（中文）](docs/PROTOCOL.zh-CN.md) · [Protocol (English, STE)](docs/PROTOCOL.md) · [测试向量](test/vectors/test-vectors.json) · [Rust 库](src/rust/README.md)

## 为什么不直接用动态二维码？

动态二维码容易实现，但很慢：一帧 QR v25 只能装约 1.2 KB，而且方形会浪费手机和显示器的大部分屏幕。Lumen Code 保留"相机对准屏幕"的简单，同时大幅提高吞吐量：

| | 动态二维码（v25，常见） | Lumen Code v1 |
|---|---|---|
| 每格比特 | 1 | 2（C4，默认）或 3（C8） |
| 形状 | 正方形 | 铺满屏幕（自适应宽高，宽高比最高 2.4） |
| 格子大小 | 由版本决定 | 按屏幕物理尺寸自动选择 |
| 每帧数据（手机屏） | 约 1.2 KB | 约 2.2 KB（C4）/ 3.4 KB（C8） |
| 每帧数据（13 寸笔记本） | 约 1.2 KB | 约 6.9 KB（C4）/ 10.3 KB（C8） |
| 丢帧 | 通常循环重放 | 喷泉码：每帧都是新数据 |
| 一个角被反光挡住 | 整帧失败 | 用其余三个角继续解码 |

默认每秒 12 帧时，笔记本屏幕的原始容量约 80–125 KB/s。实际速度取决于接收端相机，接收页面会实时显示各项数据。速度具体从哪里来，逐项量化见[性能指南](docs/guide/04-performance.zh-CN.md)。

## 试用

在线地址：<https://lumencode.awesometools.dev/zh>。在发送端屏幕打开「发送」并选择文件，在接收端手机打开「接收」。自己运行网页应用（SvelteKit，位于 [`src/website`](src/website/README.md)）：

```sh
cd src/website && bun install
bun run dev            # http://localhost:5173（localhost 下可用相机）
bun run dev:https      # https://<局域网IP>:5173，供手机使用（自签名证书）
bun run build          # 预渲染页面和 Cloudflare Worker，输出到 .svelte-kit/cloudflare
```两个页面都同时支持手机和电脑，无需安装，数据不离开设备。

> **光敏提示：** 发送时，码会以每秒最多 60 次的频率闪烁变色。对闪光敏感的人请不要注视。网页和原生 App 都会在码开始播放前显示这条提示。


- **发送端**：可发送文件，也可直接输入或粘贴文本（文件 | 文本 两个标签）；密度（低 / 自动 / 高 / 极高）、4 色或 8 色、纠错等级、帧率、引擎；轻点画面可显示/隐藏控制栏。
- **接收端**：文本和图片以全屏阅读视图打开（JSON 自动格式化并高亮，链接可点击，字号可调）；文件可保存或打开。扫描时显示进度、速度、格子在相机中的像素大小、成功率，以及一行诊断信息（引擎耗时、相机帧率、识别次数、新帧数、发送帧率、捕获率）和提示；完成后仍可查看本次传输的各项参数，并自动关闭相机。相机支持变焦时，双指在画面上缩放。每个数字的含义见[仪表解读](docs/guide/05-dashboard.zh-CN.md)。
- **引擎切换**：两个页面都可以在 JavaScript 实现与编译成 WebAssembly 的 Rust 实现之间切换（`JS | WASM`），两者生成的帧逐位一致。接收端默认用 WASM，扫描中途切换不会丢失进度。
- **码铺满屏幕**：自适应宽高，横屏竖屏都铺满；码是横的时，接收端把手机横过来即可。

## 原生应用（iOS、Android、桌面）

同样的传输，做成原生应用：三个平台共用一套 Rust 核心（[`src/app`](src/app/README.md)：应用逻辑、扫描流水线和协议库）。各平台外壳只负责绘制和调用系统能力，外观与交互和网页一致，并用上原生才有的能力：相机图像原地读取、相机支持时 60–240 帧采集、Android 上自行控制曝光、120 Hz 播放、震动反馈、屏幕亮度，以及手机发热时自动减少工作量。

| 应用 | 外壳 | 构建并运行 | 说明 |
|---|---|---|---|
| iOS 17+ | SwiftUI | `src/ios/scripts/run.sh` | [src/ios](src/ios/README.md) |
| Android 10+ | Jetpack Compose | `cd src/android && ./gradlew :app:installDebug` | [src/android](src/android/README.md) |
| macOS、Windows、Linux | GPUI | `cd src/desktop && cargo run --release` | [src/desktop](src/desktop/README.md) |

桌面版安装包在 [Releases](https://github.com/atshelchin/lumen-code/releases) 页面：已签名并经公证的 macOS 镜像，以及 Windows、Linux 压缩包。桌面版在 macOS 上经过测试；Windows、Linux 版（含摄像头接收）是新加入的，尚未在真机上验证。

各子目录的说明为英文。

## 在自己的项目里使用

不需要安装任何包：直接从本仓库取代码（建议固定到某个标签或提交）。下面的步骤写得足够明确，编程 agent 也可以照着做。

| 你要做的 | 取什么 | 再读 |
|---|---|---|
| 网页或 Node 程序 | `src/core/` 整个目录：ES 模块，无需构建，无依赖。要用更快的引擎，再加上 `dist/wasm/lumen.wasm`，由 `src/core/wasm-engine.js` 加载 | [集成指南](docs/guide/03-integration.zh-CN.md) §3.2–§3.3 |
| Rust 程序 | 在 `Cargo.toml` 里写 `lumen-code = { git = "https://github.com/atshelchin/lumen-code" }`；库名为 `lumen`（无依赖） | [src/rust/README.md](src/rust/README.md) |
| 其他语言的应用（Swift、Kotlin、C……） | 同一个 crate 的 C ABI：`src/rust/include/lumen.h`，用 `--features ffi` 构建 | [集成指南](docs/guide/03-integration.zh-CN.md) §3.5 |
| 全新的实现 | [规范](docs/PROTOCOL.zh-CN.md)和 [`test/vectors/test-vectors.json`](test/vectors/test-vectors.json) | [集成指南](docs/guide/03-integration.zh-CN.md) §3.7 |

所有集成都必须遵守以下规则（[规范](docs/PROTOCOL.zh-CN.md) §10.4、§11、§13）：

- 直接使用现成的接收端：它负责执行资源上限，并在文件校验失败时自动重新开始传输。
- 文件名和内容都来自发送端：绝不能把文件名当作路径使用，文件本身要按来源不明的文件对待。
- 码开始播放之前要提醒用户：它每秒会闪烁多次。

## 原理简述

1. 文件可选用 DEFLATE 压缩，再切分成源块；
2. 类 Raptor 喷泉码把每个源块变成无限的符号流，任意约 K+2 个不同的符号即可恢复 K 个符号的源块（实测平均比 K 多约 1.6 个）；
3. 每个符号加上 CRC 成为数据包，再编码为 Reed–Solomon 码字；
4. 码字沿 Hilbert 曲线排进网格，每个码字占一块紧凑区域（相机拍到撕裂帧时只丢少量码字）；
5. 三个定位图案、一个角标、校准点阵和四份帧头，让接收端在透视、镜头畸变、模糊和摩尔纹下仍能定位网格。

[开发者指南](docs/guide/README.zh-CN.md)配有图和具体数字，适合先建立直觉；完整的规范性描述见 [docs/PROTOCOL.zh-CN.md](docs/PROTOCOL.zh-CN.md)。

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

相机模拟器（`test/camera.js`）模拟透视、桶形畸变、光学模糊、颜色串扰、白平衡、暗角、光晕、摩尔纹、运动模糊、4:2:0 色度下采样和噪声。在随机的"手持、没对准"测试中，4 色模式约 30 帧解出 27–28 帧、码字成功率 86–89%；剩余失败的帧是每格只有约 3 个相机像素并且严重模糊的情况。

## 性能（Apple M2，1080p 相机帧）

| 识别引擎 | 手机尺寸的码 | 笔记本尺寸的码 |
|---|---|---|
| JavaScript（V8） | 14–16 ms | 41–45 ms |
| Rust → WebAssembly | 9–10 ms | 21 ms |
| Rust 原生 | 7–9 ms | 16 ms |

接收端还会跳过已解出的帧（只读帧头，JS 约 8 ms），让识别跟随发送端帧率（每个发送帧约识别 1.6 次，而不是相机每帧都识别），并只取码所在的区域、由 GPU 缩放到每格约 6 像素后再交给识别线程；视频上方不用背景模糊，收完自动关闭相机。合起来计算量降低数倍，避免手机发热。

## 状态

- 协议 1.0 版为正式版，已冻结（2026-10-20）：线格式不再改变，已发布的桌面 App v0.1.x 使用的就是它。规范覆盖线格式和接收端：合法性规则、传输状态机、资源上限、错误、安全考虑和扩展规则。
- 两个独立实现（JavaScript、Rust）通过同一套一致性向量：编码、合法与非法输入的解码、经过接收端状态机的包流。在伪随机语料（2 万个帧头、2 万个描述符、5000 个损坏的 DEFLATE 流、2000 组喷泉符号）上两者的判定完全一致。
- 目前的验证：模拟器、Chrome 虚拟摄像头，以及初步的真机测试。真机调优仍在进行，欢迎反馈效果不好的机型和环境。
- 反馈问题时请附上版本号：网页在首页底部以及发送、接收卡片底部显示（如"版本 9cc2078 · 2026-10-20"，即源码的提交 ID），App 在首页底部和接收完成后的详情里显示。
- 许可证：[MIT](LICENSE)。
- 演示页面在每个 HTML 文件末尾加载了一个流量统计脚本（`tj.appsdata.org`），自行部署时可删除这几行。
