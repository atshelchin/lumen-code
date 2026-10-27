# Privacy, rating and compliance answers

The answers both stores ask for, and why each is true. Check them against the code when the apps change.

## App Store Connect

- **App Privacy → Data collection:** "No, we do not collect data from this app." The Swift code has no URLSession, Network or analytics code; the Rust core depends only on crux_core, serde, serde_json and uniffi; the release binary imports no socket functions (checked 2026-10-27).
- **Privacy manifest** (`src/ios/Lumen/Resources/PrivacyInfo.xcprivacy`, in the app bundle): NSPrivacyTracking false, no collected data types, no tracking domains; required-reason APIs: UserDefaults (CA92.1) and file timestamp (C617.1, from Rust's standard library).
- **Export compliance:** `ITSAppUsesNonExemptEncryption = NO` (no encryption; only checksums and Reed–Solomon).
- **Age rating:** "None" for every question → 4+. Unrestricted web access: No (links open in Safari).
- **Category:** Utilities; secondary Productivity.
- **Price:** US$2.99 in the United States; let Apple derive the other storefronts. China mainland needs an ICP number; leave that storefront off until there is one.
- **Availability:** iPhone only (not iPad; not on Apple silicon Macs or Vision Pro).
- **EU DSA:** trader, MONDAY LABS LTD, with the company address, phone and email.

## Google Play Console

- **Data safety:** "No" to collecting or sharing any user data. The merged manifest has no INTERNET permission (only CAMERA and VIBRATE); no network or analytics libraries.
- **Content rating (IARC):** "Utility, productivity, communication or other"; "No" to every content question.
- **Target audience:** 13 and over; not designed for children.
- **Ads:** No.
- **App access:** restricted functionality, instructions in `review-notes.md`.
- **Government, financial, health features:** none.
- **Category:** Tools.
- **Price:** US$2.99 (choose "Paid" when creating the app; a free app can never become paid). Accept Google's converted local prices. Needs a payments profile.
- **Account:** organisation (MONDAY LABS LTD, D-U-N-S); no 12-tester closed test required.

## Both

- Privacy policy: https://lumencode.link/privacy (live). It states that the apps collect nothing and that store sales and crash reports come from Apple and Google.
- Photosensitivity: warned in the app before the first code plays, and in the store description.
