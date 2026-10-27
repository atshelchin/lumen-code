// The privacy policy and the terms of service, in English and Chinese. What they say about data
// must stay true to the code: the web app (src/website), the analytics script in src/app.html
// (Rybbit, site b9594b9f1c70; its features are set on the server, checked 2026-10-27: page views, URL parameters,
// outbound links; no replay, no errors, no web vitals, no clicks/copy/forms), and the apps
// (no network code; Android requests only CAMERA and VIBRATE). Links: [label](url).
import type { Lang } from './i18n';
import { PRICE } from './landing';

export const UPDATED = '2026-10-27';
/** The company that publishes Lumen Code, and its address for questions about these pages. */
export const COMPANY = 'MONDAY LABS LTD';
export const CONTACT = 'hello@mondaylabs.ltd';
const ISSUES = 'https://github.com/atshelchin/lumen-code/issues';

export interface LegalDoc {
	title: string;
	description: string;
	summary?: string[];
	sections: { h: string; p?: string[]; list?: string[] }[];
}

export interface Legal {
	updated: string;
	summaryTitle: string;
	home: string;
	privacy: LegalDoc;
	terms: LegalDoc;
}

const en: Legal = {
	updated: 'Last updated: ',
	summaryTitle: 'In short',
	home: 'Home',
	privacy: {
		title: 'Privacy Policy',
		description:
			'How Lumen Code handles your data: files and camera images never leave your devices; the website counts visits without cookies; the apps collect nothing.',
		summary: [
			'The files and text you send or receive never leave your devices. They are not uploaded to us or to anyone else.',
			'Camera images are processed on your device. They are never recorded or sent anywhere.',
			'The website counts visits with an open-source analytics tool, without cookies. The apps collect nothing.',
			'There are no accounts and no ads, and we do not sell or share personal data.'
		],
		sections: [
			{
				h: 'Who we are',
				p: [
					`Lumen Code is made by ${COMPANY} (“we”, “us”). This policy covers the website at lumencode.link (the web app included) and the Lumen Code apps for iPhone, Android, macOS, Windows and Linux that we publish.`
				]
			},
			{
				h: 'Your files, your text and your camera',
				p: [
					'Encoding and decoding happen entirely on your devices, in your browser or in the app. A transfer goes from one screen to one camera: the content, the file name and the camera images never reach us or any server.',
					'The camera is used only after you start receiving, and only to read the code on the other screen. You can withdraw camera access at any time in your browser or system settings.'
				]
			},
			{
				h: 'The website',
				p: [
					'**Hosting.** Cloudflare delivers the website. Like any web server, it processes your IP address and the details of each request (the page asked for, your browser) to deliver the pages and to protect them against abuse. See [Cloudflare’s privacy policy](https://www.cloudflare.com/privacypolicy/).',
					'**Visit statistics.** We count visits with [Rybbit](https://github.com/rybbit-io/rybbit), an open-source analytics tool, served from tj.appsdata.org. When a page opens, it records the page address (with its query parameters), the referring page, your browser language, screen size, browser and operating system, an approximate location derived from your IP address, and clicks on links to other websites. To count returning visits it keeps a random identifier in your browser’s local storage. It sets no cookies, does not record your screen or what you type, and never sees the files or text you transfer. The website’s security policy lets the script send data only to that server.',
					'We use these statistics only to understand how the website is used and to improve it. We do not use them for advertising, combine them with other data, sell them or share them. The web app works the same if you block the analytics script with a content blocker.',
					'**Settings in your browser.** The web app remembers your choices (language, engine, sender settings, reader view) in your browser’s local storage. They stay on your device; clearing the website’s data in your browser removes them.'
				]
			},
			{
				h: 'The apps',
				p: [
					'The apps contain no analytics, advertising or tracking code and do not send anything over the internet; the Android app does not even request internet access. They ask only for what a transfer needs: the camera, vibration on Android, and on iPhone permission to add a received image to your photos when you save it. Received files go where you choose to save them.',
					'**Purchases.** The iPhone and Android apps are sold through the App Store and Google Play. Apple and Google handle the payment under their own privacy policies; they send us sales reports, which may include your country or region and the order details, but never your payment details or anything you transfer. If you have allowed it in your device settings, Apple or Google may also send us crash reports: technical details such as the device model, the system version and where the app crashed, never your files.'
				]
			},
			{
				h: 'Email',
				p: [
					'If you write to us, we use your address and your message only to reply, and keep them only as long as the conversation needs.'
				]
			},
			{
				h: 'Children',
				p: [
					'Lumen Code is not directed at children, and we do not knowingly collect personal information from anyone, children included.'
				]
			},
			{
				h: 'Your choices and rights',
				p: [
					'Because there are no accounts, we hold no profile about you. You can block the analytics script, clear the website’s data in your browser, and withdraw camera access at any time. If you have a question or a request about your data, including the rights you have under laws such as the GDPR, contact us and we will answer.'
				]
			},
			{
				h: 'Changes',
				p: [
					'If we change how Lumen Code handles data, we will update this page and the date at the top before the change takes effect.'
				]
			},
			{
				h: 'Contact',
				p: [`Email [${CONTACT}](mailto:${CONTACT}), or open an issue on [GitHub](${ISSUES}).`]
			}
		]
	},
	terms: {
		title: 'Terms of Service',
		description:
			'The terms for using the Lumen Code website and apps: open source under the MIT License, provided as is, store apps sold once through the App Store and Google Play.',
		sections: [
			{
				h: 'Agreement',
				p: [
					'These terms apply when you use the website at lumencode.link (the web app included) or the Lumen Code apps we publish (together, “the service”). By using the service you agree to them. If you do not agree, do not use the service.'
				]
			},
			{
				h: 'Open source',
				p: [
					'The Lumen Code source code, the protocol and the apps are published under the [MIT License](https://github.com/atshelchin/lumen-code/blob/main/LICENSE). Nothing in these terms limits what that licence allows you to do with the code. These terms cover the website we run and the apps we distribute.'
				]
			},
			{
				h: 'Using the service',
				p: [
					'You may use Lumen Code to send and receive files and text that you have the right to share. You agree not to:'
				],
				list: [
					'use it to break the law or to infringe the rights of others;',
					'use it to spread malware or content that harms others;',
					'try to disrupt, overload or attack the website or the services it runs on.'
				]
			},
			{
				h: 'Your content',
				p: [
					'Transfers go directly from one of your devices to another; we never receive, see or store what you transfer, and we cannot recover it. You are responsible for the content you send and receive.'
				]
			},
			{
				h: 'Photosensitivity',
				p: [
					'While sending, the code flashes and changes colour up to 60 times a second. People who are sensitive to flashing light, for example with photosensitive epilepsy, should not look at it. You use the sender at your own risk.'
				]
			},
			{
				h: 'Purchases',
				p: [
					`The iPhone and Android apps cost ${PRICE}, paid once, and are sold through the App Store and Google Play. Payment, refunds and family sharing are governed by the terms of the store you buy from; on the App Store, Apple’s standard Licensed Application End User License Agreement applies. Prices may change for future purchases. The web version stays free, and anyone may build the apps from the source code for free.`
				]
			},
			{
				h: 'No warranty',
				p: [
					'The service is provided “as is” and “as available”, without warranties of any kind, express or implied, including merchantability, fitness for a particular purpose and non-infringement. A transfer can fail or be slow, depending on the screens, cameras and conditions; keep your own copy of anything important.'
				]
			},
			{
				h: 'Limitation of liability',
				p: [
					`To the extent the law allows, we are not liable for any indirect, incidental, special or consequential damages, or for any loss of data, profits or use, arising from the service. Our total liability for any claim is limited to the amount you paid us for the app concerned (${PRICE}), or to nothing if you paid nothing. Some laws do not allow these limits; where they apply, your rights under them are not affected.`
				]
			},
			{
				h: 'Changes and availability',
				p: [
					'We may change, suspend or end the website or the apps, and we may update these terms. We will post the new terms on this page with a new date; using the service after that means you accept them. The protocol is open and frozen, so files and apps you already have keep working with any implementation of it.'
				]
			},
			{
				h: 'Contact',
				p: [`Email [${CONTACT}](mailto:${CONTACT}), or open an issue on [GitHub](${ISSUES}).`]
			}
		]
	}
};

const zh: Legal = {
	updated: '最后更新：',
	summaryTitle: '简要说明',
	home: '首页',
	privacy: {
		title: '隐私政策',
		description:
			'Lumen Code 如何处理你的数据：文件和相机画面不离开你的设备；网站不用 Cookie 统计访问；应用不收集任何数据。',
		summary: [
			'你发送和接收的文件、文字都不离开你的设备，不会上传给我们或任何人。',
			'相机画面只在你的设备上处理，从不录制、也不发送到任何地方。',
			'网站用一个开源统计工具统计访问，不使用 Cookie。应用不收集任何数据。',
			'没有账号、没有广告，我们不出售也不分享个人数据。'
		],
		sections: [
			{
				h: '我们是谁',
				p: [
					`Lumen Code 由 ${COMPANY}（下称“我们”）开发。本政策适用于 lumencode.link 网站（包括网页应用），以及我们发布的 iPhone、Android、macOS、Windows、Linux 版 Lumen Code 应用。`
				]
			},
			{
				h: '你的文件、文字和相机',
				p: [
					'编码和解码完全在你的设备上进行：在浏览器里，或在应用里。传输只从一块屏幕到一台相机：内容、文件名和相机画面都不会到达我们或任何服务器。',
					'只有在你开始接收后才会使用相机，且只用来读取另一块屏幕上的码。你可以随时在浏览器或系统设置里收回相机权限。'
				]
			},
			{
				h: '网站',
				p: [
					'**托管。** 网站由 Cloudflare 提供。和任何网站服务器一样，它会处理你的 IP 地址和每次请求的信息（请求的页面、浏览器），用于提供页面和防范滥用。参见 [Cloudflare 隐私政策](https://www.cloudflare.com/privacypolicy/)。',
					'**访问统计。** 我们用开源统计工具 [Rybbit](https://github.com/rybbit-io/rybbit) 统计访问，脚本来自 tj.appsdata.org。打开页面时，它记录页面地址（含查询参数）、来源页面、浏览器语言、屏幕尺寸、浏览器和操作系统、根据 IP 地址推算的大致地区，以及指向其他网站的链接点击。为了统计回访，它在浏览器的本地存储里保存一个随机标识。它不设置 Cookie，不录制屏幕，不记录你的输入，也从来接触不到你传输的文件和文字。本站的安全策略只允许这个脚本把数据发往该服务器。',
					'这些统计只用于了解网站的使用情况并加以改进。我们不把它们用于广告，不与其他数据合并，不出售也不分享。用内容拦截器屏蔽统计脚本后，网页应用照常使用。',
					'**浏览器里的设置。** 网页应用会在浏览器的本地存储里记住你的选择（语言、引擎、发送设置、阅读视图）。它们只留在你的设备上；在浏览器里清除本网站的数据即可删除。'
				]
			},
			{
				h: '应用',
				p: [
					'应用不含任何统计、广告或追踪代码，也不通过互联网发送任何内容；Android 版甚至不申请联网权限。应用只申请传输所需的权限：相机；Android 上的振动；iPhone 上在你保存收到的图片时，申请“添加到照片”。收到的文件保存在你选择的位置。',
					'**购买。** iPhone 与 Android 应用通过 App Store 和 Google Play 销售，付款由苹果和谷歌按各自的隐私政策处理；它们会给我们销售报告，其中可能有你所在的国家或地区和订单信息，但绝不会有你的支付信息或你传输的内容。如果你在设备设置里允许，苹果或谷歌也可能把崩溃报告发给我们：报告里是设备型号、系统版本、崩溃位置这类技术信息，不会有你的文件。'
				]
			},
			{
				h: '邮件',
				p: ['你给我们发邮件时，我们只用你的地址和邮件内容来回复，只在沟通需要的期间保留。']
			},
			{
				h: '儿童',
				p: ['Lumen Code 不以儿童为对象；我们不会有意收集任何人（包括儿童）的个人信息。']
			},
			{
				h: '你的选择与权利',
				p: [
					'因为没有账号，我们不持有关于你的任何档案。你可以随时屏蔽统计脚本、清除浏览器中本网站的数据、收回相机权限。如果你对自己的数据有疑问或请求（包括依据《个人信息保护法》、GDPR 等法律享有的权利），请联系我们，我们会答复。'
				]
			},
			{
				h: '变更',
				p: ['如果我们改变 Lumen Code 处理数据的方式，会在变更生效前更新本页及页首日期。']
			},
			{
				h: '联系我们',
				p: [`发送邮件至 [${CONTACT}](mailto:${CONTACT})，或在 [GitHub](${ISSUES}) 上提交 issue。`]
			}
		]
	},
	terms: {
		title: '服务条款',
		description:
			'使用 Lumen Code 网站和应用的条款：MIT 许可证开源，按现状提供，商店版应用在 App Store 与 Google Play 一次买断。',
		sections: [
			{
				h: '协议',
				p: [
					'本条款适用于你使用 lumencode.link 网站（包括网页应用）或我们发布的 Lumen Code 应用（合称“本服务”）。使用本服务即表示你同意本条款；如不同意，请不要使用。'
				]
			},
			{
				h: '开源',
				p: [
					'Lumen Code 的源代码、协议和应用以 [MIT 许可证](https://github.com/atshelchin/lumen-code/blob/main/LICENSE) 发布。本条款不限制该许可证赋予你的任何权利。本条款适用于我们运营的网站和我们分发的应用。'
				]
			},
			{
				h: '使用本服务',
				p: ['你可以用 Lumen Code 收发你有权分享的文件和文字。你同意不会：'],
				list: [
					'用它违反法律或侵犯他人权利；',
					'用它传播恶意软件或伤害他人的内容；',
					'试图干扰、压垮或攻击本网站及其所依托的服务。'
				]
			},
			{
				h: '你的内容',
				p: [
					'传输直接从你的一台设备到另一台设备；我们从不接收、查看或保存你传输的内容，也无法恢复它。你对自己收发的内容负责。'
				]
			},
			{
				h: '光敏提示',
				p: [
					'发送时，码会以每秒最多 60 次的频率闪烁变色。对闪光敏感的人（例如光敏性癫痫患者）请不要注视。使用发送端的风险由你自行承担。'
				]
			},
			{
				h: '购买',
				p: [
					`iPhone 与 Android 应用售价 ${PRICE}，一次买断，通过 App Store 和 Google Play 销售。付款、退款和家人共享适用你购买所在商店的条款；在 App Store 上，适用 Apple 标准的“许可应用最终用户许可协议”。今后购买的价格可能调整。网页版继续免费，任何人也都可以从源代码免费编译应用。`
				]
			},
			{
				h: '不提供担保',
				p: [
					'本服务按“现状”和“可用”提供，不作任何明示或默示的担保，包括适销性、特定用途适用性和不侵权。传输可能因屏幕、相机和环境条件而失败或变慢；重要内容请自行保留副本。'
				]
			},
			{
				h: '责任限制',
				p: [
					`在法律允许的范围内，对于因本服务产生的任何间接、附带、特殊或后果性损害，以及数据、利润或使用上的损失，我们不承担责任。我们就任何索赔承担的全部责任，以你为相关应用向我们支付的金额（${PRICE}）为限；未付费的，则为零。部分法律不允许上述限制，在其适用时，你依法享有的权利不受影响。`
				]
			},
			{
				h: '变更与可用性',
				p: [
					'我们可能修改、暂停或终止网站或应用，也可能更新本条款。新条款会连同新日期发布在本页；此后继续使用即表示你接受新条款。协议是开放且已冻结的，你已有的文件和应用可以继续与它的任何实现配合使用。'
				]
			},
			{
				h: '联系我们',
				p: [`发送邮件至 [${CONTACT}](mailto:${CONTACT})，或在 [GitHub](${ISSUES}) 上提交 issue。`]
			}
		]
	}
};

export const LEGAL: Record<Lang, Legal> = { en, zh };

/** Text with **bold** and [links](url), cut into parts for rendering. */
export function inline(text: string): { text: string; href?: string; bold?: boolean }[] {
	const out: { text: string; href?: string; bold?: boolean }[] = [];
	const re = /\*\*([^*]+)\*\*|\[([^\]]+)\]\(([^)]+)\)/g;
	let last = 0;
	for (const m of text.matchAll(re)) {
		if (m.index! > last) out.push({ text: text.slice(last, m.index) });
		out.push(m[1] ? { text: m[1], bold: true } : { text: m[2], href: m[3] });
		last = m.index! + m[0].length;
	}
	if (last < text.length) out.push({ text: text.slice(last) });
	return out;
}
