---
title: Building from Source
description: Building Flow-Like from Source Code
sidebar:
  order: 10
---

## Get the Source Code

Head to the official [GitHub Repository](https://github.com/TM9657/flow-like) and clone the repository to your local machine:

```bash
git clone https://github.com/TM9657/flow-like.git
cd flow-like
```

Alternatively, you can fork the repository to your GitHub account and clone it from there (especially if you plan to [contribute](/dev/contribute/)).

We're continuously pushing updates to Flow-Like. The latest stable versions are available on the `main` and `alpha` branches. These are the branches that contain the source code snapshot for our [latest builds available for download](https://flow-like.com/download). They should compile without issues on your machine.

To get the *latest* changes, try the `dev` branch:
```bash
git checkout dev
```

## Install Rust
As [we are using Rust](/dev/rust/) for our backend, please continue by installing **Rust** on your machine as described in the [official Rust installation guide](https://www.rust-lang.org/tools/install).

If you already have Rust installed, make sure you have the latest stable version:
```bash
rustup update stable
```

## Install Bun
For all build scripts and to bundle the frontend, we are using [Bun](https://bun.sh/). Please install Bun by following the [official Bun installation guide](https://bun.com/docs/installation).

Alternatively, you can also install **Bun** via **npm**. To do so, first install [Node.js](https://nodejs.org/en/download/) and then run:
```bash
npm install -g bun
```

## Install Tauri Prerequisites
To build the desktop application, we are using [Tauri](https://tauri.app/).

Please make sure you've installed the necessary **System Dependencies** for your respective operating system as described in the [official Tauri Prerequisites guide](https://tauri.app/start/prerequisites/#system-dependencies).

## Install Protocol Buffer Compiler
Some of our Rust dependencies require **Protobuf**. Please install the Protobuf compiler (`protoc`) on your machine as described in the [official Protobuf installation guide](https://protobuf.dev/installation/).

## Install Node Packages
Now that you have all dependencies installed, fetch all required Node packages by running:
```bash
bun install
```

## Build and Run in Dev Mode
To build and run the Flow-Like desktop application in development mode run:
```bash
bun run dev:desktop:<os>:<arch>
```

Please replace `<os>` and `<arch>` with your respective operating system and architecture, available options are:
```bash
# Example for macOS on Apple Silicon
bun run dev:desktop:mac:arm
# Example for macOS on Intel/AMD (x64)
bun run dev:desktop:mac:intel
# Example for Windows on Intel/AMD (x64)
bun run dev:desktop:win:x64
# Example for Windows on ARM
bun run dev:desktop:win:arm
# Example for Linux on Intel/AMD (x64)
bun run dev:desktop:linux:x64
```

Running in dev mode builds the backend without the Rust `cargo build` `--release` flag. Frontend assets are bundled on each change, so you can see your changes live in the app.

## Productive Builds
To create a productive build of the Flow-Like desktop application, run the following command (no need to specify OS and architecture here):
```bash
bun run build:desktop
```

The build binary will be located at `./target/release/flow-like-desktop`. Bundled app installers can be found in `./target/release/bundle`.

## Further Build Scripts
You can find all available build scripts in the [package.json](https://github.com/TM9657/flow-like/blob/main/package.json) file of the repository root.

## Known Issues

Tauri uses `WebKit` for rendering the frontend. We've noticed some [issues with certain Linux distributions and graphics drivers combinations](https://github.com/tauri-apps/tauri/issues).