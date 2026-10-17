# Lumen Code developer guides

[中文](README.zh-CN.md) · [Protocol specification](../PROTOCOL.md) · [Project home](../../README.md)

The protocol specification is written for people who want to build a compatible implementation. It is precise, but tiring to read. These guides are for people who first want to understand how it works. They explain the intuition and the reasons, with figures and numbers, and do not try to be complete down to every byte. Read them first, and the specification becomes much easier.

| # | Guide | For whom | What you will learn |
|---|---|---|---|
| 1 | [How it works](01-how-it-works.md) | Everyone | What a frame looks like; how a file becomes cells and then a file again; why the fountain code makes dropped frames harmless |
| 2 | [How the receiver reads a frame](02-scanner.md) | People who want to improve or port the scanner | The ten steps from a camera photo to packets, and which real-world problem each step handles |
| 3 | [Integration guide](03-integration.md) | People who want to use it in their own app | Minimal examples and parameter choices for JS, WASM, Rust and the C ABI (Android / iOS / desktop) |
| 4 | [Performance: where the speed comes from](04-performance.md) | People who care about throughput | Where it is faster than animated QR codes (quantified item by item), what limits the speed, and how to make it faster next |
| 5 | [Reading the dashboard](05-dashboard.md) | People who use the demo pages or investigate "why is it slow" | What each number on the sender and receiver means, healthy ranges, and what to adjust for what you see |

Suggested reading order: 1 → 5 → 4. Read 3 when you start building, and 2 when you need to change the scanner.

## The thirty-second version

- The sender cuts the file into small blocks and uses a **fountain code** to generate a packet stream that never repeats. Each packet gets a CRC and then **Reed–Solomon** ECC, and becomes a **codeword**.
- Each frame is a grid of coloured cells (2 bits per cell by default) that holds a dozen to several dozen codewords. The grid adapts its width and height to fill the screen, and the cell size follows the physical size of the screen.
- When the receiver captures any frame, it keeps every codeword it can decode from it. Codewords are independent of each other, so even a half-blurred frame is useful.
- Whichever frames are lost, the file can be restored once the number of distinct packets received exceeds the number of symbols in the file by two or three. There is no "wait for the next pass to fill in that frame".
