# nvdash

A lightweight, native NVIDIA GPU monitor for ML workloads. No web views, no Electron — just a compact GPU peek widget built with [egui](https://github.com/emilk/egui) and [nvml-wrapper](https://github.com/Cldfire/nvml-wrapper). Lives in the system tray.

![nvdash](util-pic.png)

## Features

- **System tray widget** — lives in the Windows notification area; click to toggle, right-click to quit
- **Close to tray** — closing the window hides it; the app keeps polling in the background
- **Compact UI (~380x240)** — everything visible at a glance, no scrolling
- **Text sparklines** — GPU % and VRAM history using block characters (last 60s)
- **Heat-colored temp bar** — temperature bar with sage-to-rose color gradient
- **Process list** — top 3 GPU processes by VRAM usage
- **Clock summary** — GFX, MEM, SM clocks and fan speed in a single line
- **Driver/CUDA info** — hover the GPU name for driver and CUDA versions
- **Bottom bar controls** — pin (always-on-top), frameless mode, poll rate, opacity
- **Configurable polling** — 250ms / 500ms / 1s / 2s intervals

## Requirements

- Windows (system tray integration uses Win32 APIs)
- NVIDIA GPU with drivers installed
- [NVML](https://developer.nvidia.com/management-library-nvml) (ships with NVIDIA drivers)

## Build

```
cargo build --release
```

## Run

```
cargo run --release
```

Or run the binary directly:

```
./target/release/nvdash.exe
```

## Usage

1. Launch nvdash — the window opens and a green icon appears in the system tray
2. **Click X** — window hides to tray, app keeps running
3. **Left-click tray icon** — toggle the window on/off (positions above the tray)
4. **Right-click tray icon** — "Quit" menu to exit

## License

MIT
