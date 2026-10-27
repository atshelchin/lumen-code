# Notes for app review

Paste into App Store Connect (App Review Information → Notes) and Play Console (App content → App access → "All or some functionality is restricted", with these instructions). Replace the video link before submitting.

```
Lumen Code sends a file from one screen to a phone's camera. Receiving needs a second screen that shows a Lumen code. No account or login is needed.

To test receiving with one phone:
1. On any computer, open https://lumencode.link/send in a browser, choose a file (or the Text tab and type something), and confirm the photosensitivity notice.
2. On the phone, open Lumen Code, tap Receive, allow the camera, and point it at the computer screen so the whole code is visible.
3. The progress bar fills; when it reaches 100% the file opens (text in a reader, other files with Save/Share).

To test sending: tap Send, choose a file or type text, confirm the notice; the screen plays the code. The website's receiver (https://lumencode.link/recv) or a second phone with the app can read it.

A video of a complete transfer: <VIDEO LINK>

"Open in Lumen Code" (iOS) / sharing a file to the app (Android) starts sending that file.

The code flashes up to 60 times a second; the app shows a photosensitivity warning and waits for confirmation before the first code plays.

The app makes no network connections, has no account and collects no data. The protocol and the app are open source (MIT): https://github.com/atshelchin/lumen-code
```
