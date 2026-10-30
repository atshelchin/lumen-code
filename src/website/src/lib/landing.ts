// Everything the home page says, in English and Chinese. Written for people who know the field:
// every number states its conditions and comes from the protocol (docs/PROTOCOL.md, incl.
// Appendix C), the measurements (docs/guide/04-performance.md) or the code; the limits come from
// the protocol's security considerations (§13). Getting started stays three plain steps.
import type { Lang } from './i18n';

/** One price worldwide for the iPhone and Android apps, paid once; each store converts it. */
export const PRICE = 'US$2.99';
/** Store pages, once the apps are published; until then the page says "coming soon". */
export const STORES: { appStore: string | null; googlePlay: string | null } = {
	appStore: null,
	googlePlay: null
};

interface Plan {
	name: string;
	price: string;
	per: string;
	points: string[];
	action?: string;
}

interface Item {
	title: string;
	text: string;
}

export interface Landing {
	nav: { start: string; apps: string; how: string; speed: string; limits: string; pricing: string };
	hero: {
		eyebrow: string;
		title: [string, string];
		lead: string;
		send: string;
		recv: string;
		try: string;
		desktop: string;
		note: string;
	};
	start: { title: string; steps: Item[] };
	how: { title: string; flow: [string, string][]; items: Item[] };
	speed: {
		title: string;
		head: [string, string, string, string];
		rows: [string, string, string, string][];
		note: string;
		more: string;
	};
	limits: { title: string; lead: string; items: Item[] };
	open: { title: string; items: Item[] };
	apps: { title: string; lead: string; items: { name: string; req: string; text: string }[] };
	pricing: { title: string; lead: string; build: Plan; store: Plan; note: string };
	faq: { title: string; items: { q: string; a: string }[] };
	cta: { title: string; text: string };
	soon: string;
	license: string;
}

const en: Landing = {
	nav: {
		start: 'Get started',
		apps: 'Apps',
		how: 'How it works',
		speed: 'Performance',
		limits: 'Limits',
		pricing: 'Pricing'
	},
	hero: {
		eyebrow: 'Open protocol 1.0 · MIT · Web, iPhone, Android, desktop',
		title: ['Send files from', 'a screen to a camera'],
		lead: 'The sender shows the file as a sequence of colour-coded frames; another device films them with its camera and rebuilds the file. No network, pairing or account.',
		send: 'Send a file',
		recv: 'Receive',
		try: 'Open this site on both devices: Send on one, Receive on the other (a phone’s browser is fine).',
		desktop: 'Download for desktop',
		note: `The web and desktop versions are free. iPhone and Android apps: ${PRICE}, coming soon.`
	},
	start: {
		title: 'Get started',
		steps: [
			{
				title: 'Send',
				text: 'On the device with the file, open this site (or the desktop app), tap Send and choose a file or type text.'
			},
			{
				title: 'Receive',
				text: 'On the other device, open this site in the browser, tap Receive and keep the whole code in the camera view.'
			},
			{ title: 'Save', text: 'At 100 % the file is checked and ready to open, save or share.' }
		]
	},
	how: {
		title: 'How it works',
		flow: [
			['file', 'DEFLATE if smaller'],
			['fountain code', 'blocks ≤ 16 384 symbols'],
			['packets', 'CRC-16'],
			['codewords', 'Reed–Solomon'],
			['frames', '4–60 per second']
		],
		items: [
			{
				title: 'Frame',
				text: 'Cell size follows the screen’s physical size (estimated from CSS pixels on the web); 2 bits per cell (4 colours) or 3 (8 colours). Three finders and a corner mark: any three of the four corners locate it. Alignment markers follow lens distortion.'
			},
			{
				title: 'Reed–Solomon',
				text: 'Every codeword is RS over GF(2⁸) with 10, 16 or 22 % parity (16 % by default). Codewords lie along a Hilbert curve, so a capture torn between two frames loses only a few of them.'
			},
			{
				title: 'Fountain code',
				text: 'Raptor-like, over GF(2). Any K + e distinct symbols of a block decode it; e averages 1.6 up to K = 1000 and 2.4 at K = 16 384. Missed frames and any order are fine; nothing is retransmitted.'
			},
			{
				title: 'Four levels of checks',
				text: 'The 70-bit frame header in four copies with CRC-16; Reed–Solomon per codeword; CRC-16 per packet; size and CRC-32 of the whole file. A file that fails is never delivered: the transfer restarts.'
			}
		]
	},
	speed: {
		title: 'Performance',
		head: ['', 'Animated QR v25-L', 'Phone screen (example)', '13″ laptop (example)'],
		rows: [
			['Grid (cells)', '117 × 117', '83 × 147', '143 × 254'],
			['Bits per cell', '1', '2', '2 / 3'],
			['Payload per frame', '1.24 KB', '2.2 KB', '6.9 / 10.3 KB'],
			['Raw rate at 12 fps', '15 KB/s', '27 KB/s', '82 / 124 KB/s']
		],
		note: 'The sender plays 4–60 fps (12 by default). Best end-to-end result so far, measured on real phones: 96 KB/s, Android app to iPhone app. What a receiver achieves depends on its camera; the receive screen shows the live rate and what limits it.',
		more: 'Methodology and measurements'
	},
	limits: {
		title: 'Limits',
		lead: 'From the protocol’s security considerations (§13), and what to expect in practice:',
		items: [
			{
				title: 'Not confidential',
				text: 'Frames are not encrypted. Anyone who can see the screen, or a recording of it, can read the file. Encrypt it first if that matters.'
			},
			{
				title: 'Not authenticated',
				text: 'The CRCs catch accidental errors, not tampering. Sign the file if its origin matters.'
			},
			{
				title: 'Tens of KB/s',
				text: 'Good for documents, keys, configuration, photos. 1 MB takes about 20 s at 50 KB/s. The reference receivers accept up to 256 MiB.'
			},
			{
				title: 'Line of sight',
				text: 'The camera needs at least 3.4 pixels per cell, 4.5 or more to be steady. Glare, steep angles and blur slow it down.'
			},
			{
				title: 'Flashing',
				text: 'The code changes up to 60 times a second; people sensitive to flashing light should not look at it.'
			}
		]
	},
	open: {
		title: 'Open, frozen, tested',
		items: [
			{
				title: 'Frozen at 1.0',
				text: 'The wire format will not change. The specification is in English and Chinese.'
			},
			{
				title: 'Two implementations',
				text: 'JavaScript and Rust pass the same conformance vectors and make identical decisions on 20 000 headers, 20 000 descriptors, 5 000 damaged DEFLATE streams and 2 000 fountain symbol sets.'
			},
			{
				title: 'No package',
				text: 'Take src/core for the web, the Rust crate, or its C ABI. A coding agent can follow the integration guide.'
			},
			{ title: 'MIT License', text: 'The protocol, both implementations and all the apps.' }
		]
	},
	apps: {
		title: 'Apps',
		lead: 'The web and desktop versions are free; the phone apps are coming soon. Every version sends to and receives from every other.',
		items: [
			{
				name: 'Web',
				req: 'Any browser with a camera',
				text: 'Open the page and use it; nothing to install. JavaScript or Rust/WebAssembly engine.'
			},
			{
				name: 'iPhone',
				req: 'iOS 17 or later',
				text: 'Uses the camera directly, at up to 240 frames a second, so it usually receives faster than the browser. Plays codes at 120 Hz.'
			},
			{
				name: 'Android',
				req: 'Android 10 or later',
				text: 'Uses the camera directly, at up to 60 frames a second, so it usually receives faster than the browser. Its own exposure control avoids washed-out colours and blur.'
			},
			{
				name: 'Desktop',
				req: 'macOS, Windows, Linux',
				text: 'Free download. Send from a computer screen, or receive with a webcam.'
			}
		]
	},
	pricing: {
		title: 'Pricing',
		lead: 'The web version is free. The store app and the one you build yourself are the same app; buying it saves the build and supports Lumen Code’s maintenance.',
		build: {
			name: 'Build it yourself',
			price: 'Free',
			per: 'forever',
			points: ['Needs Android Studio, or Xcode on a Mac'],
			action: 'How to build'
		},
		store: { name: 'Google Play and App Store', price: PRICE, per: 'once', points: [] },
		note: 'Android and iPhone are bought separately.'
	},
	faq: {
		title: 'Questions',
		items: [
			{
				q: 'Does the file go through a server?',
				a: 'No. It goes only from the sending screen to the receiving camera. The website counts page visits (see the privacy policy); the apps never connect to the network.'
			},
			{
				q: 'Does it work without internet?',
				a: 'The apps work fully offline. The web version needs a connection to open the page; the transfer itself does not use the network.'
			},
			{
				q: 'The web version is free. Do I need the app?',
				a: 'No. The apps read the camera directly, so they are faster than the browser. Buying one pays for further development.'
			},
			{
				q: 'Why not AirDrop or a cable?',
				a: 'If you can use them, do. Lumen Code is for when you can’t: devices from different makers, no shared network, an isolated computer or one with USB disabled, or a screen you can only look at.'
			}
		]
	},
	cta: {
		title: 'Try it with two devices you already have.',
		text: 'Open this site on both: Send on one, Receive on the other.'
	},
	soon: 'Coming soon',
	license: 'Open source under the MIT License.'
};

const zh: Landing = {
	nav: { start: '上手', apps: '应用', how: '原理', speed: '性能', limits: '局限', pricing: '定价' },
	hero: {
		eyebrow: '开放协议 1.0 · MIT · 网页、iPhone、Android、桌面',
		title: ['用屏幕和相机', '传文件'],
		lead: '发送端把文件显示成一串彩色编码帧，另一台设备用相机拍下来，还原出文件。不需要网络、配对和账号。',
		send: '发送文件',
		recv: '接收',
		try: '两台设备都打开本网站：一台点“发送”，另一台点“接收”（手机浏览器即可）。',
		desktop: '下载桌面版',
		note: `网页版和桌面版免费。iPhone 与 Android 应用 ${PRICE}，即将上架。`
	},
	start: {
		title: '上手',
		steps: [
			{
				title: '发送',
				text: '在有文件的设备上打开本网站（或桌面版），点“发送”，选择文件或输入文字。'
			},
			{
				title: '接收',
				text: '在另一台设备的浏览器里打开本网站，点“接收”，让码完整出现在相机画面里。'
			},
			{ title: '保存', text: '到 100 % 时文件已校验，可以打开、保存或分享。' }
		]
	},
	how: {
		title: '原理',
		flow: [
			['文件', '更小时用 DEFLATE'],
			['喷泉码', '每块 ≤ 16 384 个符号'],
			['数据包', 'CRC-16'],
			['码字', 'Reed–Solomon'],
			['帧', '每秒 4–60 帧']
		],
		items: [
			{
				title: '帧',
				text: '格子大小按屏幕物理尺寸确定（网页版按 CSS 像素估算），每格 2 bit（4 色）或 3 bit（8 色）。三个定位图案加一个角标，看到任意三个角即可定位；对齐点阵跟随镜头畸变。'
			},
			{
				title: 'Reed–Solomon',
				text: '每个码字是 GF(2⁸) 上的 RS 码，校验占 10 %、16 % 或 22 %（默认 16 %）。码字沿 Hilbert 曲线排布，相机拍到两帧之间的撕裂画面时只损失少量码字。'
			},
			{
				title: '喷泉码',
				text: '类 Raptor 码，基于 GF(2)。一个块收到任意 K + e 个不同符号即可解出；K ≤ 1000 时 e 平均 1.6，K = 16 384 时平均 2.4。丢帧、乱序都不影响，不需要重传。'
			},
			{
				title: '四层校验',
				text: '70 位帧头四份副本加 CRC-16；每个码字 Reed–Solomon；每个包 CRC-16；整个文件校验长度和 CRC-32。校验不过的文件不会交付，传输重新开始。'
			}
		]
	},
	speed: {
		title: '性能',
		head: ['', '动态二维码 v25-L', '手机屏幕（示例）', '13 英寸笔记本（示例）'],
		rows: [
			['网格（格）', '117 × 117', '83 × 147', '143 × 254'],
			['每格比特', '1', '2', '2 / 3'],
			['每帧有效数据', '1.24 KB', '2.2 KB', '6.9 / 10.3 KB'],
			['12 fps 原始速率', '15 KB/s', '27 KB/s', '82 / 124 KB/s']
		],
		note: '发送端 4–60 fps（默认 12）。目前真机端到端最好成绩：96 KB/s，Android 应用发送、iPhone 应用接收。接收端实际能到多少取决于相机，接收界面会实时显示速率和限速环节。',
		more: '测量方法与数据'
	},
	limits: {
		title: '局限',
		lead: '来自协议的安全考虑（§13）和实际使用：',
		items: [
			{
				title: '不保密',
				text: '帧没有加密。能看到屏幕（或录像）的人都能读出文件。需要保密时先加密再发送。'
			},
			{ title: '不认证', text: 'CRC 只能发现意外错误，防不了篡改。需要确认来源时对文件签名。' },
			{
				title: '每秒几十 KB',
				text: '适合文档、密钥、配置、照片。按 50 KB/s 计算 1 MB 约 20 秒。参考接收端单个文件上限 256 MiB。'
			},
			{
				title: '需要看得清',
				text: '相机画面里每格至少 3.4 像素，4.5 以上才稳定。反光、斜拍和模糊都会降低速度。'
			},
			{ title: '会闪烁', text: '码每秒最多变化 60 次，对闪光敏感的人请勿注视。' }
		]
	},
	open: {
		title: '开放、冻结、可验证',
		items: [
			{ title: '1.0 已冻结', text: '线格式不会再变。规范有中英文两版。' },
			{
				title: '两个实现',
				text: 'JavaScript 与 Rust 通过同一套一致性测试向量，并在 20 000 个帧头、20 000 个描述符、5 000 个损坏的 DEFLATE 流和 2 000 组喷泉符号上做出完全相同的判断。'
			},
			{
				title: '不用装包',
				text: '网页取 src/core，原生用 Rust crate 或它的 C ABI。编程 agent 可以照着接入指南完成集成。'
			},
			{ title: 'MIT 许可证', text: '协议、两个实现和所有应用。' }
		]
	},
	apps: {
		title: '应用',
		lead: '网页版和桌面版免费，手机应用即将上架。各版本之间都能互相收发。',
		items: [
			{
				name: '网页',
				req: '任何带相机的浏览器',
				text: '打开网页就能用，不用安装；可选 JavaScript 或 Rust/WebAssembly 引擎。'
			},
			{
				name: 'iPhone',
				req: 'iOS 17 及以上',
				text: '直接调用相机，最高每秒 240 帧，接收通常比网页版快；以 120 Hz 播放码。'
			},
			{
				name: 'Android',
				req: 'Android 10 及以上',
				text: '直接调用相机，最高每秒 60 帧，接收通常比网页版快；曝光由应用控制，减少过曝和拖影。'
			},
			{
				name: '桌面',
				req: 'macOS、Windows、Linux',
				text: '免费下载。用电脑屏幕发送，或用摄像头接收。'
			}
		]
	},
	pricing: {
		title: '定价',
		lead: '网页版免费。商店版和自己编译的是同一个应用；买商店版省去编译，也支持 Lumen Code 的持续维护。',
		build: {
			name: '自己编译',
			price: '免费',
			per: '永久',
			points: ['需要 Android Studio，或 Mac 上的 Xcode'],
			action: '怎样编译'
		},
		store: { name: 'Google Play 与 App Store', price: PRICE, per: '一次性', points: [] },
		note: 'Android 和 iPhone 需要分别购买。'
	},
	faq: {
		title: '常见问题',
		items: [
			{
				q: '文件会上传到服务器吗？',
				a: '不会。文件只从发送端屏幕传到接收端相机。网站只统计页面访问量（见隐私政策），应用完全不联网。'
			},
			{
				q: '没有网络能用吗？',
				a: '应用可以完全离线使用。网页版需要联网打开页面，传输本身不用网络。'
			},
			{
				q: '网页版免费，还需要买应用吗？',
				a: '不需要。应用直接读取相机画面，比网页版快。买应用是在支持后续开发。'
			},
			{
				q: '有隔空投送或数据线，为什么还用它？',
				a: '能用它们就用它们。Lumen Code 适合用不了的情况：两台设备品牌不同、没有共同网络、电脑被隔离或禁用了 USB，或者只能看到对方的屏幕。'
			}
		]
	},
	cta: { title: '用手边的两台设备试试。', text: '两台设备都打开本网站，一台发送，一台接收。' },
	soon: '即将上架',
	license: '以 MIT 许可证开源。'
};

export const LANDING: Record<Lang, Landing> = { en, zh };
