# 上架 App Store 与 Google Play

iPhone 和 Android 应用以 MONDAY LABS LTD 的名义上架，售价 US$2.99，一次买断。本文按顺序列出要做的事：账号和协议只能由你在网页上完成，标注了“仓库”的项由代码改动完成。审查清单的依据见 `docs/STATUS.md`。

## 0 两个商店都要的前提

1. **网站。** 已上线（2026-10-27 核对：`/privacy`、`/terms`、`/send` 都能打开）。两家都要求隐私政策网址能打开，审核员测试接收也会用到网页发送端。
2. **公司的 D-U-N-S 编号。** 苹果和谷歌的公司账号都要用它核实 MONDAY LABS LTD。没有的话在 dnb.com 或苹果的 D-U-N-S 查询页免费申请，通常一到两周。
3. **一段演示视频。** 一台设备播放、另一台手机接收、收完打开文件，30–60 秒，上传到能直接打开的地址（YouTube 不公开链接或网站上的文件）。审核员没有第二台设备时靠它判断功能。
4. **仓库：上架前必须修的问题。**
   - iOS：加 `PrivacyInfo.xcprivacy`（UserDefaults 原因 CA92.1；Rust 标准库带入的文件元数据调用，原因 C617.1；不追踪、不收集数据）。缺了它上传会被拒（ITMS-91053）。
   - iOS：首页文案里提到 Android（违反 2.3.10），iOS 上改用不提其他平台的说法。
   - Android：发布版目前用调试密钥签名，Play 会拒收。按第 2 节第 1 步建上传密钥，并在 `build.gradle.kts` 里读取 `keystore.properties`。
   - 两端：从其他应用“用 Lumen 打开”的文件会直接开始播放，跳过光敏提示，要改成首次播放前必须确认一次。
   - 两端：真机审查发现的问题（控制栏挡住码导致收不到、关闭文字发送后残留的文字可以再次发出、完成页没有关闭按钮等），见下一轮 UI 修改。
5. **商店材料。** 已写好：`app-store/listing.md`、`google-play/listing.md`（名称、副标题、关键词、简短说明，字数已核对）、`description-en.txt` / `description-zh.txt`（完整说明）、`review-notes.md`（审核说明，提交前把演示视频链接填进去）、`privacy-and-compliance.md`（隐私、分级、合规问卷的答案及依据）。还要做：真机截图（界面简化完成后拍）、Google Play 的 512 图标和 1024×500 宣传图。

## 1 App Store

1. **开发者账号。** 在 developer.apple.com/programs 以公司（Organization）身份注册 Apple Developer Program，填 D-U-N-S，年费 US$99。现有团队 F9W689P9NE 如果已经是 MONDAY LABS LTD 的公司账号，直接用。
2. **协议、税务、银行。** App Store Connect → 商务（Business）：接受付费 App 协议（Paid Apps Agreement），填公司银行账户和税表（非美国公司一般是 W-8BEN-E）。没完成这一步，付费应用无法上架。
3. **欧盟交易商身份。** App Store Connect 里按《数字服务法》（DSA）声明交易商身份，公开公司地址、电话和邮箱；不声明就不能在欧盟上架。
4. **新建 App。** 先在 developer.apple.com → Certificates, IDs & Profiles → Identifiers 注册 Bundle ID `ltd.mondaylabs.lumencode`（Xcode 自动签名也会代为注册）。然后 App Store Connect → 我的 App → “+” 新建 App：平台 iOS，名称 “Lumen Code”（被占用时可加副标题式后缀，如 “Lumen Code: Screen to Camera”），主要语言，Bundle ID 选 `ltd.mondaylabs.lumencode`，SKU 例如 `lumencode-ios`。Bundle ID 第一次上传后不能改。
5. **只上 iPhone。** 工程已设为仅 iPhone，并关闭了在 Apple 芯片 Mac 和 Vision Pro 上运行 iPhone 版；App Store Connect 的“定价与销售范围”里也确认这两项未勾选。只需要 6.9 英寸 iPhone 截图。
6. **价格与地区。** 定价与销售范围 → 价格：基准国家选美国，US$2.99，其他地区用苹果自动换算。中国大陆商店要求 ICP 备案号；没有备案就先不选中国大陆。
7. **App 隐私。** App 隐私 → 数据收集：选“我们不从此 App 中收集数据”。隐私政策网址填 `https://lumencode.link/privacy`。
8. **年龄分级。** 所有问题选“无”，得到 4+。“无限制网络访问”选否。
9. **商店页。** 中英文各一套：副标题、宣传文本、描述（写明需要第二块屏幕和光敏提示）、关键词、支持网址、营销网址；6.9 英寸 iPhone 截图（1320×2868）。分类：工具（Utilities）。
10. **出口合规。** 工程里已设置 `ITSAppUsesNonExemptEncryption = NO`，上传后不会再问。
11. **打包上传。** 先在仓库根目录确保工作区干净（否则应用里显示的版本带 `+dev`），然后：
    ```sh
    cd src/ios
    scripts/build-core.sh        # Rust 核心，Release
    xcodegen generate            # 生成 Lumen.xcodeproj
    open Lumen.xcodeproj
    ```
    Xcode 里选 “Any iOS Device (arm64)” → Product → Archive → Distribute App → App Store Connect → Upload。每次上传前把 `project.yml` 里的 `CURRENT_PROJECT_VERSION` 加 1。
12. **TestFlight。** 构建处理完（十几分钟到一小时）后出现在 TestFlight，先装到自己的 iPhone 上，实际收发一次。
13. **审核信息。** App 审核信息 → 备注：粘贴 `review-notes.md`（测试步骤、网页发送端地址、演示视频链接、“打开方式”的说明、不联网不收集数据）。联系人填你的姓名、电话、邮箱。不需要演示账号。
14. **提交审核。** 选择构建版本 → 添加以供审核 → 提交。通常一到三天有结果；被拒时看“解决方案中心”的具体条款，修改后重新提交。
15. **发布。** 可选“审核通过后自动发布”或手动发布。

## 2 Google Play

1. **上传密钥。** 生成一次，连同备份离线保存，丢了要走重置流程：
   ```sh
   keytool -genkeypair -keystore lumencode-upload.jks -alias upload -keyalg RSA -keysize 4096 -validity 10000
   ```
   然后建 `src/android/keystore.properties`（不进 git），写 `storeFile`、`storePassword`、`keyAlias`、`keyPassword`。仓库要相应改 `app/build.gradle.kts`，读取这个文件给 release 签名（参考 Keyper 的 `app-android/app/build.gradle.kts`）。
2. **开发者账号。** play.google.com/console 注册，选“组织”账号，填 D-U-N-S，一次性费用 US$25，完成身份验证。组织账号不需要“12 名测试者连续 14 天封闭测试”这一步（2023 年 11 月后注册的个人账号需要）。
3. **付款资料。** 设置 → 付款资料：建商家（Payments）资料，填公司银行账户和税务信息。付费应用必须有它。
4. **新建应用。** 所有应用 → 创建应用：名称、默认语言、类型“应用”，**付费**。注意：免费应用以后不能改成付费，这一步一定选付费。
5. **应用包名。** `ltd.mondaylabs.lumencode`，第一次上传后不能改。应用名称 “Lumen Code”。
6. **应用内容（政策 → 应用内容），逐项填写：**
   - 隐私政策：`https://lumencode.link/privacy`。
   - 应用访问权限：选“部分或全部功能受限”，填写 `review-notes.md` 里的测试说明（接收需要另一块屏幕，用网页发送端或演示视频）。
   - 广告：不含广告。
   - 内容分级（IARC 问卷）：类别选“实用工具、效率、通讯或其他”，问题全部选否。
   - 目标受众：13 岁以上（或 18 岁以上），不面向儿童。
   - 数据安全：不收集也不分享任何用户数据（应用没有联网权限，已核对合并后的清单）。
   - 政府应用、金融功能、健康：都选否或不适用。
7. **商店页。** 中英文：应用名称（30 字符）、简短说明（80 字符）、完整说明（4000 字符，写明光敏提示）、512×512 图标、1024×500 宣传图、至少 2 张手机截图；不锁屏幕方向时建议加平板截图。分类：工具。联系邮箱 hello@mondaylabs.ltd，网站 https://lumencode.link。
8. **价格。** 获利 → 应用价格：US$2.99，其他国家用谷歌换算的价格。选择上架的国家和地区。
9. **打包：**
   ```sh
   cd src/android
   ./gradlew :app:bundleRelease   # Gradle 会先运行 scripts/build-core.sh 构建 Rust 核心
   # 产物：app/build/outputs/bundle/release/app-release.aab
   ```
   每次上传把 `versionCode` 加 1。
10. **Play 应用签名。** 第一次上传 AAB 时接受 Play 应用签名（Play App Signing）；之后在“测试和发布 → 应用完整性”里能看到应用签名证书的 SHA-256，建议写进 README，方便别人核对安装包。
11. **内部测试。** 测试和发布 → 测试 → 内部测试：新建版本，上传 AAB，把自己加为测试者，从 Play 安装后实际收发一次。
12. **正式发布。** 测试和发布 → 正式版：从内部测试推广版本或新建版本，写版本说明，发送审核。首次审核一般几天到一周。

## 3 两边都上架以后

- 在 `src/website/src/lib/landing.ts` 的 `STORES` 里填上两个商店链接，首页的“即将上架”会变成商店按钮。
- 给发布用的提交打标签（例如 `git tag ios-1.0.0`、`android-1.0.0`），商店版本可以对应到源码。
- 回复商店评论里的问题；崩溃报告在 App Store Connect 和 Play Console 的“质量”页。
