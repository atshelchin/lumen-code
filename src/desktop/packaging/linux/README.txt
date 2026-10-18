Lumen Code for Linux
====================

Run ./lumen. The app needs a Vulkan driver (mesa-vulkan-drivers or your GPU vendor's)
and a Wayland or X11 session.

To add it to your applications menu:

  install -Dm755 lumen ~/.local/bin/lumen
  install -Dm644 com.shelchin.LumenCode.desktop ~/.local/share/applications/com.shelchin.LumenCode.desktop
  install -Dm644 com.shelchin.LumenCode.png ~/.local/share/icons/hicolor/256x256/apps/com.shelchin.LumenCode.png

Receiving uses a Video4Linux webcam (/dev/video*); your user needs access to it (on most
distributions the "video" group). Source and other platforms:
https://github.com/atshelchin/lumen-code
