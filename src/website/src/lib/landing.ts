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
	nav: { start: string; how: string; speed: string; limits: string; pricing: string };
	hero: {
		eyebrow: string;
		title: [string, string];
		lead: string;
		send: string;
		recv: string;
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
	pricing: { title: string; lead: string; web: Plan; build: Plan; store: Plan; note: string };
	faq: { title: string; items: { q: string; a: string }[] };
	cta: { title: string; text: string };
	soon: string;
	license: string;
}

const en: Landing = {
	nav: {
		start: 'Get started',
		how: 'How it works',
		speed: 'Performance',
		limits: 'Limits',
		pricing: 'Pricing'
	},
	hero: {
		eyebrow: 'Open protocol 1.0 · MIT · Web, iPhone, Android, desktop',
		title: ['Send files from', 'a screen to a camera'],
		lead: 'The sender shows the file as a sequence of colour-coded frames; a phone or webcam films them and rebuilds the file. One-way: no network, no pairing, no return channel.',
		send: 'Send a file',
		recv: 'Receive',
		note: `Free in the browser. iPhone and Android apps: ${PRICE}, coming soon.`
	},
	start: {
		title: 'Get started',
		steps: [
			{
				title: 'Send',
				text: 'On the device with the file, open Send and choose a file or type text.'
			},
			{
				title: 'Receive',
				text: 'On the other device, open Receive and keep the whole code in the camera view.'
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
			['frames', '12–60 per second']
		],
		items: [
			{
				title: 'Frame',
				text: 'A grid sized to the physical screen, 2 bits per cell (4 colours) or 3 (8 colours). Three finders and a corner mark: any three of the four corners locate it. Alignment markers follow lens distortion.'
			},
			{
				title: 'Reed–Solomon',
				text: 'Every codeword is RS over GF(2⁸) with 10–30 % parity (16 % by default). Codewords lie along a Hilbert curve, so a capture torn between two frames loses only a few of them.'
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
		head: ['', 'Animated QR v25-L', 'Phone screen', '13″ laptop screen'],
		rows: [
			['Grid (cells)', '117 × 117', '83 × 147', '143 × 254'],
			['Bits per cell', '1', '2', '2 / 3'],
			['Payload per frame', '1.24 KB', '2.2 KB', '6.9 / 10.3 KB'],
			['Raw rate at 12 fps', '15 KB/s', '27 KB/s', '82 / 124 KB/s']
		],
		note: 'The sender plays 12–60 fps. Best end-to-end result so far: 96 KB/s, Android app to iPhone app. What a receiver achieves depends on its camera; the receive screen shows the live rate and what limits it.',
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
				text: 'The camera needs about 3.5 or more pixels per cell. Glare, steep angles and blur slow it down.'
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
		lead: 'Every version speaks the same protocol and sends to every other.',
		items: [
			{
				name: 'Web',
				req: 'A browser with a camera',
				text: 'JavaScript or Rust/WebAssembly engine; nothing to install.'
			},
			{
				name: 'iPhone',
				req: 'iOS 17 or later',
				text: 'Camera up to 240 fps when the sender is fast; 120 Hz playback.'
			},
			{
				name: 'Android',
				req: 'Android 10 or later',
				text: 'Camera up to 60 fps with the app’s own exposure control.'
			},
			{
				name: 'Desktop',
				req: 'macOS, Windows, Linux',
				text: 'Send from a large screen, receive with a webcam.'
			}
		]
	},
	pricing: {
		title: `Free in the browser. ${PRICE} for the apps.`,
		lead: 'The same protocol and the same files; the apps read the camera directly and are faster.',
		web: {
			name: 'Web',
			price: 'Free',
			per: 'forever',
			points: ['Any browser with a camera', 'No install, no account'],
			action: 'Open the web app'
		},
		build: {
			name: 'Build it yourself',
			price: 'Free',
			per: 'forever',
			points: ['Full source, MIT License', 'Desktop builds on GitHub Releases'],
			action: 'How to build'
		},
		store: {
			name: 'App Store and Google Play',
			price: PRICE,
			per: 'once',
			points: [
				'Signed apps for iPhone and Android',
				'No subscription, ads or account',
				'Pays for further development'
			]
		},
		note: 'One price worldwide, shown in your currency. iPhone and Android are bought separately.'
	},
	faq: {
		title: 'Questions',
		items: [
			{
				q: 'Does the file go through a server?',
				a: 'No. The page loads from this site; the file goes only from the screen to the camera. The site counts page visits (see the privacy policy); the apps send nothing.'
			},
			{
				q: 'Does it work offline?',
				a: 'The apps need no connection. The web version needs one only to load the page.'
			},
			{
				q: 'Why pay for an app?',
				a: 'You don’t have to. The apps read the camera directly and are faster than the browser; buying one pays for development.'
			},
			{
				q: 'Why not AirDrop or a cable?',
				a: 'Use them when you can. This is for when you can’t: different makers, no shared network, an air-gapped or locked-down machine, a screen you can only look at.'
			}
		]
	},
	cta: {
		title: 'Try it with two devices you already have.',
		text: 'Open Send on one and Receive on the other.'
	},
	soon: 'Coming soon',
	license: 'Open source under the MIT License.'
};

const zh: Landing = {
	nav: { start: '上手', how: '原理', speed: '性能', limits: '局限', pricing: '定价' },
	hero: {
		eyebrow: '开放协议 1.0 · MIT · 网页、iPhone、Android、桌面',
		title: ['用屏幕和相机', '传文件'],
		lead: '发送端把文件显示成一串彩色编码帧，手机或摄像头拍下来还原文件。单向传输：不需要网络和配对，也没有回传通道。',
		send: '发送文件',
		recv: '接收',
		note: `网页版免费。iPhone 与 Android 应用 ${PRICE}，即将上架。`
	},
	start: {
		title: '上手',
		steps: [
			{ title: '发送', text: '在有文件的设备上打开“发送”，选择文件或输入文字。' },
			{ title: '接收', text: '在另一台设备上打开“接收”，让码完整出现在相机画面里。' },
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
			['帧', '每秒 12–60 帧']
		],
		items: [
			{
				title: '帧',
				text: '网格按屏幕物理尺寸确定，每格 2 bit（4 色）或 3 bit（8 色）。三个定位图案加一个角标，看到任意三个角即可定位；对齐点阵跟随镜头畸变。'
			},
			{
				title: 'Reed–Solomon',
				text: '每个码字是 GF(2⁸) 上的 RS 码，校验占 10–30 %（默认 16 %）。码字沿 Hilbert 曲线排布，相机拍到两帧之间的撕裂画面时只损失少量码字。'
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
		head: ['', '动态二维码 v25-L', '手机屏幕', '13 英寸笔记本'],
		rows: [
			['网格（格）', '117 × 117', '83 × 147', '143 × 254'],
			['每格比特', '1', '2', '2 / 3'],
			['每帧有效数据', '1.24 KB', '2.2 KB', '6.9 / 10.3 KB'],
			['12 fps 原始速率', '15 KB/s', '27 KB/s', '82 / 124 KB/s']
		],
		note: '发送端 12–60 fps。目前端到端最好成绩：96 KB/s，Android 应用发送、iPhone 应用接收。接收端实际能到多少取决于相机，接收界面会实时显示速率和限速环节。',
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
				text: '相机画面里每格需要约 3.5 像素以上。反光、斜拍和模糊都会降低速度。'
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
		lead: '各版本用同一套协议，可以互相收发。',
		items: [
			{
				name: '网页',
				req: '带相机的浏览器',
				text: 'JavaScript 或 Rust/WebAssembly 引擎，无需安装。'
			},
			{ name: 'iPhone', req: 'iOS 17 及以上', text: '发送端够快时相机最高 240 帧；120 Hz 播放。' },
			{ name: 'Android', req: 'Android 10 及以上', text: '相机最高 60 帧，由应用自己控制曝光。' },
			{ name: '桌面', req: 'macOS、Windows、Linux', text: '大屏发送，摄像头接收。' }
		]
	},
	pricing: {
		title: `网页免费，应用 ${PRICE}`,
		lead: '同一套协议，传同样的文件；应用直接读取相机，速度更快。',
		web: {
			name: '网页版',
			price: '免费',
			per: '永久',
			points: ['带相机的浏览器', '无需安装和账号'],
			action: '打开网页版'
		},
		build: {
			name: '自己编译',
			price: '免费',
			per: '永久',
			points: ['完整源码，MIT 许可证', '桌面版在 GitHub Releases'],
			action: '怎样编译'
		},
		store: {
			name: 'App Store 与 Google Play',
			price: PRICE,
			per: '一次性',
			points: ['iPhone 与 Android 签名应用', '不订阅、无广告、无需账号', '用于后续开发']
		},
		note: '全球统一定价，按当地货币显示。iPhone 和 Android 需分别购买。'
	},
	faq: {
		title: '常见问题',
		items: [
			{
				q: '文件会经过服务器吗？',
				a: '不会。网页从本站加载，文件只从屏幕传到相机。本站统计页面访问（见隐私政策），应用不发送任何数据。'
			},
			{ q: '没网能用吗？', a: '应用完全不需要网络；网页版只在打开页面时需要。' },
			{ q: '为什么买应用？', a: '不买也行。应用直接读取相机，比浏览器快；购买的钱用于后续开发。' },
			{
				q: '为什么不用隔空投送或数据线？',
				a: '能用就用它们。这个用在用不了的时候：不同品牌的设备、没有共同网络、物理隔离或受管控的电脑、只能看的屏幕。'
			}
		]
	},
	cta: { title: '用手边的两台设备试试。', text: '一台打开发送，一台打开接收。' },
	soon: '即将上架',
	license: '以 MIT 许可证开源。'
};

export const LANDING: Record<Lang, Landing> = { en, zh };
