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
	facts: string[];
	how: {
		title: string;
		lead: string;
		flow: [string, string][];
		steps: { title: string; text: string }[];
	};
	speed: {
		title: string;
		lead: string;
		head: [string, string, string];
		rows: [string, string, string][];
		note: string;
	};
	features: { title: string; lead: string; items: { title: string; text: string }[] };
	apps: {
		title: string;
		lead: string;
		items: { name: string; req: string; text: string }[];
		caption: string;
	};
	open: { title: string; lead: string; items: { title: string; text: string }[] };
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
		lead: 'The sender plays your file as an animated colour code; the receiver films it and rebuilds the file. No network, no Bluetooth, no pairing, no account: the file goes from one screen to one camera, and nowhere else.',
		send: 'Send a file',
		recv: 'Receive',
		note: `Free in the browser. Apps for iPhone and Android: ${PRICE}, coming soon.`
	},
	facts: [
		'No network',
		'No pairing, no account',
		'Files never touch a server',
		'Protocol 1.0, frozen',
		'MIT licence'
	],
	how: {
		title: 'How it works',
		lead: 'One screen plays, one camera watches. Nothing goes back, so a receiver can join at any moment and miss any frame.',
		flow: [
			['holiday-photos.zip', 'your file'],
			['Fountain code', 'endless new packets'],
			['Colour frames', '12–60 a second'],
			['Camera', 'any order, any gaps'],
			['holiday-photos.zip', 'checked, byte for byte']
		],
		steps: [
			{
				title: 'Open Send',
				text: 'On the device that has the file, choose the file or type a message. The screen plays the code, sized for that screen.'
			},
			{
				title: 'Point a camera',
				text: 'Open Receive on the other device and point its camera at the screen. Three of the four corners are enough.'
			},
			{
				title: 'Hold steady',
				text: 'Every frame carries new data. A missed or blurred frame costs only its own share; the progress keeps growing until there is enough.'
			},
			{
				title: 'Save',
				text: 'The receiver checks the whole file and opens it: text and images in a reader, any other file to save or open.'
			}
		]
	},
	speed: {
		title: 'More data in every frame',
		lead: 'Animated QR codes are easy to make but slow. Lumen Code keeps the “point a camera at a screen” simplicity and fills each frame with more:',
		head: ['', 'Animated QR (v25)', 'Lumen Code 1.0'],
		rows: [
			['Bits per cell', '1', '2 (4 colours) or 3 (8 colours)'],
			['Shape', 'square', 'fills the screen, either orientation'],
			['Data per frame, phone screen', '~1.2 KB', '~2.2 KB / ~3.4 KB'],
			['Data per frame, 13″ laptop', '~1.2 KB', '~6.9 KB / ~10.3 KB'],
			['A missed frame', 'wait for it to come round', 'no matter: every frame is new'],
			['Glare on one corner', 'frame lost', 'read from the other three']
		],
		note: 'At 12 frames a second a laptop screen sends 80–125 KB/s of raw capacity. The fastest measured transfer, between two phones running the apps, reached 96 KB/s. What a receiver gets depends on its camera; the receiver shows the live numbers.'
	},
	features: {
		title: 'Made for real cameras',
		lead: 'Shaky hands, glare, cheap lenses, screens of every size: the code is designed around what goes wrong.',
		items: [
			{
				title: 'Adaptive density',
				text: 'The cell size follows the physical size of the screen: a small phone still reads reliably, a laptop carries several times more.'
			},
			{
				title: 'Fountain code',
				text: 'Every frame is new data. Frames can be missed, blurred or out of order; once enough have arrived, the file is complete.'
			},
			{
				title: 'Three corners are enough',
				text: 'A finder in each corner and an alignment grid: a corner hidden by glare or a finger, and the frame still decodes.'
			},
			{
				title: 'Colour and error correction',
				text: '2 or 3 bits per cell, Reed–Solomon on every codeword, a checksum on every packet and on the whole file.'
			},
			{
				title: 'Two engines, identical frames',
				text: 'The JavaScript and Rust/WebAssembly implementations make bit-identical frames. Switch while receiving and nothing is lost.'
			},
			{
				title: 'A dashboard that explains itself',
				text: 'Speed, cell size, success and capture rate while you scan, with a hint whenever something limits the speed.'
			}
		]
	},
	apps: {
		title: 'In the browser, or as an app',
		lead: 'They all speak the same protocol: send from a browser and receive on a phone app, or the other way round.',
		items: [
			{
				name: 'Web',
				req: 'Any modern browser with a camera',
				text: 'Nothing to install. Send and receive files and text, on phones and computers.'
			},
			{
				name: 'iPhone',
				req: 'iOS 17 or later',
				text: 'Reads the camera directly and switches it to 60–240 frames a second when the sender is fast; plays at 120 Hz, with haptics.'
			},
			{
				name: 'Android',
				req: 'Android 10 or later',
				text: 'Captures at up to 60 frames a second under the app’s own exposure control; scans less when the phone gets hot.'
			},
			{
				name: 'Desktop',
				req: 'macOS, Windows, Linux',
				text: 'Send from a big screen or receive with a webcam. Signed and notarized on macOS.'
			}
		],
		caption:
			'The apps are usually faster than the browser: they read the camera directly and use higher frame rates.'
	},
	open: {
		title: 'An open protocol, frozen at 1.0',
		lead: 'Lumen Code is a specification first. Anyone can implement it, and nothing about it will change under you.',
		items: [
			{
				title: 'Frozen',
				text: 'Version 1.0 is final: its wire format will not change. Every 1.0 receiver reads every 1.0 sender, now and later.'
			},
			{
				title: 'Two reference implementations',
				text: 'JavaScript and Rust pass the same conformance vectors and make identical decisions on tens of thousands of random and damaged inputs.'
			},
			{
				title: 'Add it to your app',
				text: 'There is no package to install: point your coding agent at the repository. The guides cover the web, Rust, and other languages through a C ABI.'
			},
			{
				title: 'Open source',
				text: 'The protocol, both implementations and all the apps, under the MIT License.'
			}
		]
	},
	pricing: {
		title: `Free in the browser. ${PRICE} for the apps.`,
		lead: 'Every version speaks the same protocol and moves the same files. The store apps add convenience and native speed.',
		web: {
			name: 'Web',
			price: 'Free',
			per: 'forever',
			points: [
				'Any modern browser with a camera',
				'Nothing to install, no account',
				'Files and text, send and receive'
			],
			action: 'Open the web app'
		},
		build: {
			name: 'Build it yourself',
			price: 'Free',
			per: 'forever',
			points: [
				'The complete source, under the MIT License',
				'iPhone, Android and desktop: the same apps as in the stores',
				'Desktop builds ready to download from GitHub Releases'
			],
			action: 'How to build'
		},
		store: {
			name: 'App Store and Google Play',
			price: PRICE,
			per: 'once',
			points: [
				'Official signed apps for iPhone and Android',
				'Updates through the store',
				'No subscription, no in-app purchases, no ads, no account',
				'Supports the continued development of Lumen Code'
			]
		},
		note: 'One price worldwide; each store shows it in your currency. iPhone and Android are bought separately, as the two stores require.'
	},
	faq: {
		title: 'Questions',
		items: [
			{
				q: 'If the browser version is free, why pay for an app?',
				a: 'You do not have to: the web version sends and receives the same files, and you can build the apps yourself for free. The store apps read the camera directly, capture at higher frame rates and control exposure, so they are usually faster and run cooler. Buying one also keeps the project maintained.'
			},
			{
				q: 'Does my file go through a server?',
				a: 'No. The web page is loaded from the site once; the file itself goes only from the sending screen to the receiving camera. Nothing is uploaded and there is no account.'
			},
			{
				q: 'How large can a file be?',
				a: 'Up to 256 MiB per transfer. In practice the speed sets the limit: at 50 KB/s, 1 MB takes about 20 seconds and 20 MB about 7 minutes.'
			},
			{
				q: 'What do I need?',
				a: 'Two devices: one with a screen, one with a camera. Phones, tablets and computers all work, in either role. In a browser the camera needs the HTTPS site (or localhost).'
			},
			{
				q: 'Does it work without the internet?',
				a: 'The apps need no connection at all. The web version needs one to load the page; the transfer itself never uses the network.'
			},
			{
				q: 'Is the flashing safe to look at?',
				a: 'While sending, the code changes colour up to 60 times a second. People who are sensitive to flashing light, for example with photosensitive epilepsy, should not look at it.'
			},
			{
				q: 'Can I use Lumen Code in my own app?',
				a: 'Yes. The protocol is open and frozen, and the code is MIT-licensed. Take src/core for the web, the Rust crate for native code, or the C ABI from other languages; the developer guides and the conformance vectors show how.'
			},
			{
				q: 'Why not AirDrop or a cable?',
				a: 'Use them when you can. Lumen Code is for when you cannot: devices of different makes, no shared network, a locked-down or air-gapped computer, or a screen you can only look at.'
			}
		]
	},
	cta: {
		title: 'Try it now, with two devices you already have.',
		text: 'Open Send on one and Receive on the other. Nothing to install.'
	},
	soon: 'Coming soon',
	license: 'Open source under the MIT License.'
};

const zh: Landing = {
	nav: { how: '原理', speed: '速度', apps: '应用', pricing: '定价', faq: '常见问题' },
	hero: {
		eyebrow: '开放协议 · 无需网络 · 网页、iPhone、Android、桌面',
		lead: '发送端把文件播放成一串彩色动态码，接收端用相机拍摄并还原文件。无需网络、蓝牙、配对或账号：文件只从一块屏幕传到一台相机，不去别处。',
		send: '发送文件',
		recv: '接收',
		note: `网页版免费。iPhone 与 Android 应用 ${PRICE}，即将上架。`
	},
	facts: ['无需网络', '无需配对和账号', '文件不经过服务器', '协议 1.0 已冻结', 'MIT 许可证'],
	how: {
		title: '原理',
		lead: '一块屏幕播放，一台相机观看。没有任何回传，所以接收端随时可以加入，漏掉哪一帧都没关系。',
		flow: [
			['holiday-photos.zip', '你的文件'],
			['喷泉码', '源源不断的新数据包'],
			['彩色帧', '每秒 12–60 帧'],
			['相机', '顺序随意，丢帧无妨'],
			['holiday-photos.zip', '逐字节校验']
		],
		steps: [
			{
				title: '打开发送',
				text: '在有文件的设备上选择文件或输入文字，屏幕开始播放编码，码的大小按这块屏幕自动调整。'
			},
			{ title: '对准相机', text: '在另一台设备上打开接收，用相机对准屏幕。四个角看到三个就够了。' },
			{
				title: '保持稳定',
				text: '每一帧都是新数据。漏掉或拍糊的帧只损失它自己那一份，进度会一直增长，直到收够为止。'
			},
			{
				title: '保存',
				text: '接收端校验整个文件后打开：文字和图片直接阅读，其他文件可以保存或打开。'
			}
		]
	},
	speed: {
		title: '每一帧装更多数据',
		lead: '动态二维码容易做，但很慢。Lumen Code 保留了“相机对准屏幕”的简单，同时让每一帧装下更多：',
		head: ['', '动态二维码（v25）', 'Lumen Code 1.0'],
		rows: [
			['每格比特数', '1', '2（4 色）或 3（8 色）'],
			['形状', '正方形', '铺满屏幕，横竖皆可'],
			['每帧数据，手机屏幕', '约 1.2 KB', '约 2.2 KB / 约 3.4 KB'],
			['每帧数据，13 英寸笔记本', '约 1.2 KB', '约 6.9 KB / 约 10.3 KB'],
			['漏掉一帧', '等它循环回来', '无所谓：每帧都是新数据'],
			['一个角反光', '整帧作废', '靠另外三个角照样读出']
		],
		note: '每秒 12 帧时，笔记本屏幕的原始容量约 80–125 KB/s。实测最快的一次是两台手机之间用应用传输，达到 96 KB/s。实际速度取决于接收端相机，接收页面会实时显示各项数据。'
	},
	features: {
		title: '为真实的相机而设计',
		lead: '手抖、反光、廉价镜头、大小不一的屏幕：这套码就是围绕这些麻烦设计的。',
		items: [
			{
				title: '自适应密度',
				text: '格子大小按屏幕的物理尺寸选择：小屏手机也能稳定识别，笔记本屏幕能多装好几倍。'
			},
			{ title: '喷泉码', text: '每一帧都是新数据。丢帧、拍糊、乱序都不要紧，收够了文件就完整了。' },
			{
				title: '三个角就够',
				text: '四角各有定位标记，再加一套对齐点阵：一个角被反光或手指挡住，整帧照样解码。'
			},
			{
				title: '多色编码与纠错',
				text: '每格 2 或 3 bit，每个码字 Reed–Solomon 纠错，每个数据包和整个文件都有校验。'
			},
			{
				title: '双引擎，帧逐位一致',
				text: 'JavaScript 与 Rust/WebAssembly 两套实现生成逐位相同的帧，接收途中切换引擎也不丢进度。'
			},
			{
				title: '看得懂的仪表盘',
				text: '扫描时显示速度、格子大小、成功率和捕获率，哪里限制了速度就给出提示。'
			}
		]
	},
	apps: {
		title: '浏览器里用，或者装个应用',
		lead: '它们说的是同一套协议：浏览器发送、手机应用接收，反过来也行。',
		items: [
			{
				name: '网页',
				req: '任何带相机的现代浏览器',
				text: '无需安装。手机和电脑都能收发文件和文字。'
			},
			{
				name: 'iPhone',
				req: 'iOS 17 及以上',
				text: '直接读取相机画面，发送端够快时把相机切到每秒 60–240 帧；120 Hz 播放，带触感反馈。'
			},
			{
				name: 'Android',
				req: 'Android 10 及以上',
				text: '最高每秒 60 帧采集，由应用自己控制曝光；手机发热时自动少扫一些。'
			},
			{
				name: '桌面',
				req: 'macOS、Windows、Linux',
				text: '用大屏幕发送，或用摄像头接收。macOS 版经过签名和苹果公证。'
			}
		],
		caption: '应用通常比浏览器快：它们直接读取相机画面，能用更高的帧率。'
	},
	open: {
		title: '开放协议，1.0 已冻结',
		lead: 'Lumen Code 首先是一份规范。任何人都可以实现它，它也不会在你脚下改变。',
		items: [
			{
				title: '已冻结',
				text: '1.0 是最终版本，线格式不会再变。每个 1.0 接收端都能读每个 1.0 发送端，现在如此，以后也如此。'
			},
			{
				title: '两个参考实现',
				text: 'JavaScript 与 Rust 通过同一套一致性测试向量，在数万个随机和损坏的输入上做出完全相同的判断。'
			},
			{
				title: '集成进你的应用',
				text: '不需要安装任何包：让你的编程 agent 读这个仓库即可。指南覆盖网页、Rust，以及通过 C ABI 接入的其他语言。'
			},
			{ title: '开源', text: '协议、两个实现和所有应用，都以 MIT 许可证发布。' }
		]
	},
	pricing: {
		title: `网页免费，应用 ${PRICE}`,
		lead: '每个版本都说同一套协议，传同样的文件。商店版应用多的是省事和原生的速度。',
		web: {
			name: '网页版',
			price: '免费',
			per: '永久',
			points: ['任何带相机的现代浏览器', '无需安装，无需账号', '文件和文字都能收发'],
			action: '打开网页版'
		},
		build: {
			name: '自己编译',
			price: '免费',
			per: '永久',
			points: [
				'完整源码，MIT 许可证',
				'iPhone、Android 和桌面：与商店版相同的应用',
				'桌面版可直接从 GitHub Releases 下载'
			],
			action: '怎样编译'
		},
		store: {
			name: 'App Store 与 Google Play',
			price: PRICE,
			per: '一次性',
			points: [
				'iPhone 与 Android 官方签名应用',
				'通过商店更新',
				'不订阅、不内购、无广告、无需账号',
				'支持 Lumen Code 的持续开发'
			]
		},
		note: '全球统一定价，商店会按你所在地区的货币显示。iPhone 和 Android 需分别购买，这是两家商店的规定。'
	},
	faq: {
		title: '常见问题',
		items: [
			{
				q: '网页版免费，为什么还要买应用？',
				a: '不买也完全可以：网页版收发的是同样的文件，应用也可以自己免费编译。商店版应用直接读取相机画面、帧率更高、能控制曝光，通常更快、更不容易发热；购买也能支持项目持续维护。'
			},
			{
				q: '我的文件会经过服务器吗？',
				a: '不会。网页只是从网站加载一次；文件本身只从发送端屏幕传到接收端相机，不上传任何东西，也没有账号。'
			},
			{
				q: '文件最大能多大？',
				a: '每次传输最大 256 MiB。实际限制来自速度：按 50 KB/s 计算，1 MB 约 20 秒，20 MB 约 7 分钟。'
			},
			{
				q: '需要什么设备？',
				a: '两台设备：一台有屏幕，一台有相机。手机、平板、电脑都可以，发送和接收都行。在浏览器里，相机需要 HTTPS 网站（或 localhost）。'
			},
			{
				q: '没有网能用吗？',
				a: '应用完全不需要网络。网页版需要联网加载页面；传输本身从不使用网络。'
			},
			{
				q: '闪烁的画面安全吗？',
				a: '发送时，码会以每秒最多 60 次的频率变色。对闪光敏感的人（例如光敏性癫痫患者）请不要注视。'
			},
			{
				q: '能用到我自己的应用里吗？',
				a: '可以。协议开放且已冻结，代码采用 MIT 许可证。网页用 src/core，原生代码用 Rust crate，其他语言用 C ABI；开发者指南和一致性测试向量会告诉你怎么做。'
			},
			{
				q: '为什么不用隔空投送或数据线？',
				a: '能用就用它们。Lumen Code 用在用不了的时候：不同品牌的设备、没有共同的网络、受管控或物理隔离的电脑，或者一块只能看的屏幕。'
			}
		]
	},
	cta: {
		title: '现在就试试，用你手边的两台设备。',
		text: '一台打开发送，一台打开接收。无需安装。'
	},
	soon: '即将上架',
	license: '以 MIT 许可证开源。'
};

export const LANDING: Record<Lang, Landing> = { en, zh };
