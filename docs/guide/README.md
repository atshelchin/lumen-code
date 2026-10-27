# Lumen Code developer guides

[中文](README.zh-CN.md) · [Protocol specification](../PROTOCOL.md) · [Project home](../../README.md)

The protocol specification is written for people building a compatible implementation: it is precise, but tiring to read. These guides explain how the system works and why, with figures and numbers, without covering every byte. Read them first and the specification becomes much easier.

| # | Guide | For whom | What you will learn |
|---|---|---|---|
| 1 | [How it works](01-how-it-works.md) | Everyone | What a frame looks like; how a file becomes cells and then a file again; why the fountain code makes dropped frames harmless |
| 2 | [How the receiver reads a frame](02-scanner.md) | People who want to improve or port the scanner | The ten steps from a camera photo to packets, and which real-world problem each step handles |
| 3 | [Integration guide](03-integration.md) | People who want to use it in their own app | Minimal examples and parameter choices for JS, WASM, Rust and the C ABI (Android / iOS / desktop) |
| 4 | [Performance: where the speed comes from](04-performance.md) | People who care about throughput | Where it is faster than animated QR codes (quantified item by item), what limits the speed, and how to make it faster next |
| 5 | [Reading the dashboard](05-dashboard.md) | People who use the demo pages or investigate "why is it slow" | What each number on the sender and receiver means, healthy ranges, and what to adjust for what you see |

Suggested reading order: 1 → 5 → 4. Read 3 when you start building, and 2 when you need to change the scanner.

## The thirty-second version

- The sender cuts the file into small blocks and uses a **fountain code** to generate a packet stream that never repeats. Each packet, with a CRC and **Reed–Solomon** ECC added, becomes a **codeword**.
- Each frame is a grid of coloured cells (2 bits per cell by default) holding a dozen to several dozen codewords. The grid's width and height adapt to fill the screen, and the cell size follows the physical size of the screen.
- The receiver keeps every codeword it can decode from any frame it captures. Codewords are independent, so even a half-blurred frame is useful.
- Whichever frames are lost, the file is restored once the receiver has two or three more distinct packets than the file has symbols. It never waits for a missed frame to come round again.
