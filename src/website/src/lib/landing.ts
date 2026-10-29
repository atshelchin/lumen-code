// Everything the home page says, in English and Chinese. Numbers and claims come from the README,
// the protocol (docs/PROTOCOL.md) and the measurements in docs/guide/04-performance.md.
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

export interface Landing {
	nav: { how: string; speed: string; apps: string; pricing: string; faq: string };
	hero: { eyebrow: string; lead: string; send: string; recv: string; note: string };
	how: { title: string; flow: [string, string][]; steps: { title: string; text: string }[] };
	speed: {
		title: string;
		lead: string;
		head: [string, string, string];
		rows: [string, string, string][];
		note: string;
	};
	features: { title: string; items: { title: string; text: string }[] };
	apps: { title: string; lead: string; items: { name: string; req: string; text: string }[] };
	open: { title: string; items: { title: string; text: string }[] };
	pricing: { title: string; lead: string; web: Plan; build: Plan; store: Plan; note: string };
	faq: { title: string; items: { q: string; a: string }[] };
	cta: { title: string; text: string };
	soon: string;
	license: string;
}

const en: Landing = {
	nav: { how: 'How it works', speed: 'Speed', apps: 'Apps', pricing: 'Pricing', faq: 'FAQ' },
	hero: {
		eyebrow: 'Open protocol · No network · Web, iPhone, Android, desktop',
		lead: 'One screen plays the file as a colour code; a camera reads it back. No network, pairing or account.',
		send: 'Send a file',
		recv: 'Receive',
		note: `Free in the browser. iPhone and Android apps: ${PRICE}, coming soon.`
	},
	how: {
		title: 'How it works',
		flow: [
			['holiday-photos.zip', 'your file'],
			['Fountain code', 'new packets'],
			['Colour frames', '12–60 a second'],
			['Camera', 'any order'],
			['holiday-photos.zip', 'checked']
		],
		steps: [
			{ title: 'Send', text: 'Choose a file or type text. The screen plays the code.' },
			{ title: 'Point a camera', text: 'Open Receive on the other device and aim at the screen.' },
			{ title: 'Save', text: 'When the file arrives, open, save or share it.' }
		]
	},
	speed: {
		title: 'More data per frame',
		lead: 'Used like an animated QR code, with more data in every frame:',
		head: ['', 'Animated QR (v25)', 'Lumen Code 1.0'],
		rows: [
			['Bits per cell', '1', '2 or 3'],
			['Data per frame, 13″ laptop', '~1.2 KB', '~6.9 KB / ~10.3 KB'],
			['A missed frame', 'wait for it to come round', 'no wait: every frame is new'],
			['Glare on one corner', 'frame lost', 'read from the other three']
		],
		note: 'Fastest measured: 96 KB/s between two phones running the apps. The real rate depends on the camera.'
	},
	features: {
		title: 'Made for real cameras',
		items: [
			{
				title: 'Adaptive density',
				text: 'Cell size follows the screen, so small phones read reliably.'
			},
			{ title: 'Fountain code', text: 'Missed or blurred frames only slow the transfer a little.' },
			{
				title: 'Three corners are enough',
				text: 'A corner hidden by glare or a finger does not stop a frame.'
			},
			{
				title: 'Two engines, identical frames',
				text: 'JavaScript and Rust/WebAssembly; switch without losing progress.'
			}
		]
	},
	apps: {
		title: 'In the browser, or as an app',
		lead: 'All of them send to each other.',
		items: [
			{ name: 'Web', req: 'A browser with a camera', text: 'Nothing to install.' },
			{ name: 'iPhone', req: 'iOS 17 or later', text: 'Camera at up to 240 fps, 120 Hz playback.' },
			{
				name: 'Android',
				req: 'Android 10 or later',
				text: 'Camera at up to 60 fps with its own exposure control.'
			},
			{
				name: 'Desktop',
				req: 'macOS, Windows, Linux',
				text: 'Send from a big screen, receive with a webcam.'
			}
		]
	},
	open: {
		title: 'An open protocol, frozen at 1.0',
		items: [
			{ title: 'Frozen', text: 'The 1.0 wire format will not change.' },
			{
				title: 'Two implementations',
				text: 'JavaScript and Rust pass the same conformance vectors.'
			},
			{
				title: 'Add it to your app',
				text: 'No package to install: point your coding agent at the repository.'
			},
			{ title: 'Open source', text: 'Protocol, implementations and apps under the MIT License.' }
		]
	},
	pricing: {
		title: `Free in the browser. ${PRICE} for the apps.`,
		lead: 'Same protocol, same files. The apps are faster.',
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
				q: 'Why pay for the app if the web version is free?',
				a: 'You don’t have to. The apps read the camera directly, so they are faster and run cooler; buying one funds development.'
			},
			{
				q: 'Does my file go through a server?',
				a: 'No. It goes only from the screen to the camera.'
			},
			{
				q: 'How large can a file be?',
				a: 'Up to 256 MiB. At 50 KB/s, 1 MB takes about 20 seconds.'
			},
			{
				q: 'Does it work offline?',
				a: 'The apps need no connection. The web version needs one only to load the page.'
			},
			{
				q: 'Is the flashing safe?',
				a: 'The code flashes up to 60 times a second. People sensitive to flashing light should not look at it.'
			},
			{
				q: 'Can I use it in my own app?',
				a: 'Yes: the protocol is open and frozen, and the code is MIT-licensed. See the developer guides.'
			}
		]
	},
	cta: {
		title: 'Try it with two devices you already have.',
		text: 'Send on one, receive on the other.'
	},
	soon: 'Coming soon',
	license: 'Open source under the MIT License.'
};

const zh: Landing = {
	nav: { how: '原理', speed: '速度', apps: '应用', pricing: '定价', faq: '常见问题' },
	hero: {
		eyebrow: '开放协议 · 无需网络 · 网页、iPhone、Android、桌面',
		lead: '屏幕把文件播放成彩色码，相机拍下来还原。不需要网络、配对或账号。',
		send: '发送文件',
		recv: '接收',
		note: `网页版免费。iPhone 与 Android 应用 ${PRICE}，即将上架。`
	},
	how: {
		title: '原理',
		flow: [
			['holiday-photos.zip', '你的文件'],
			['喷泉码', '不断生成新包'],
			['彩色帧', '每秒 12–60 帧'],
			['相机', '顺序随意'],
			['holiday-photos.zip', '校验无误']
		],
		steps: [
			{ title: '发送', text: '选文件或输入文字，屏幕开始播放。' },
			{ title: '对准', text: '另一台设备打开接收，相机对准屏幕。' },
			{ title: '保存', text: '收完后打开、保存或分享。' }
		]
	},
	speed: {
		title: '每帧装更多数据',
		lead: '用法和动态二维码一样，每帧数据更多：',
		head: ['', '动态二维码（v25）', 'Lumen Code 1.0'],
		rows: [
			['每格比特数', '1', '2 或 3'],
			['每帧数据，13 英寸笔记本', '约 1.2 KB', '约 6.9 KB / 10.3 KB'],
			['漏掉一帧', '等它循环回来', '不用等：每帧都是新数据'],
			['一个角反光', '整帧作废', '靠另外三个角读出']
		],
		note: '实测最快 96 KB/s（两台手机之间用应用）。实际速度取决于相机。'
	},
	features: {
		title: '为实际拍摄条件设计',
		items: [
			{ title: '自适应密度', text: '格子大小随屏幕调整，小屏也能稳定识别。' },
			{ title: '喷泉码', text: '漏帧、拍糊只会让传输稍慢一点。' },
			{ title: '三个角就够', text: '一个角被反光或手指挡住也能解码。' },
			{ title: '双引擎，帧逐位一致', text: 'JavaScript 与 Rust/WebAssembly，切换不丢进度。' }
		]
	},
	apps: {
		title: '浏览器里用，或者装应用',
		lead: '各版本之间都能互传。',
		items: [
			{ name: '网页', req: '带相机的浏览器', text: '无需安装。' },
			{ name: 'iPhone', req: 'iOS 17 及以上', text: '相机最高 240 帧，120 Hz 播放。' },
			{ name: 'Android', req: 'Android 10 及以上', text: '相机最高 60 帧，自己控制曝光。' },
			{ name: '桌面', req: 'macOS、Windows、Linux', text: '大屏发送，摄像头接收。' }
		]
	},
	open: {
		title: '开放协议，1.0 已冻结',
		items: [
			{ title: '已冻结', text: '1.0 的线格式不会再变。' },
			{ title: '两个实现', text: 'JavaScript 与 Rust 通过同一套一致性向量。' },
			{ title: '集成进你的应用', text: '不用装包：让编程 agent 读这个仓库。' },
			{ title: '开源', text: '协议、实现和应用均为 MIT 许可证。' }
		]
	},
	pricing: {
		title: `网页免费，应用 ${PRICE}`,
		lead: '同一套协议，传同样的文件。应用更快。',
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
				q: '网页版免费，为什么买应用？',
				a: '不买也行。应用直接读取相机，更快、发热更少；购买的钱用于后续开发。'
			},
			{ q: '文件会经过服务器吗？', a: '不会，只从屏幕传到相机。' },
			{ q: '文件最大多大？', a: '256 MiB。按 50 KB/s 计算，1 MB 约 20 秒。' },
			{ q: '没网能用吗？', a: '应用完全不需要网络；网页版只在打开页面时需要。' },
			{ q: '闪烁安全吗？', a: '码每秒最多闪烁 60 次，对闪光敏感的人请勿注视。' },
			{ q: '能集成到我的应用吗？', a: '可以：协议开放且已冻结，代码为 MIT 许可证，见开发者指南。' }
		]
	},
	cta: { title: '用手边的两台设备试试。', text: '一台发送，一台接收。' },
	soon: '即将上架',
	license: '以 MIT 许可证开源。'
};

export const LANDING: Record<Lang, Landing> = { en, zh };
