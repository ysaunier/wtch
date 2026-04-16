# Installation

## Download

Pre-built binaries are available on the [Releases](https://github.com/ysaunier/wtch/releases) page for:

- **Windows** (.exe / .msi)
- **macOS** (.dmg) - Apple Silicon and Intel
- **Linux** (.deb / .AppImage)

## Build from source

### Prerequisites

| Tool | Version | Install |
|------|---------|---------|
| Node.js | 18+ | [nodejs.org](https://nodejs.org/) |
| Rust | 1.70+ | [rustup.rs](https://rustup.rs/) |
| Tauri CLI | 2.x | `cargo install tauri-cli` |

#### Linux only

```bash
sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
```

#### macOS only

Xcode Command Line Tools:

```bash
xcode-select --install
```

### Build

```bash
git clone https://github.com/ysaunier/wtch.git
cd wtch
npm install
cargo tauri build
```

The output binary is in `src-tauri/target/release/`.

### Development

```bash
npm install
make dev
```

## Configuration

On first launch, wtch creates a config file at:

| OS | Path |
|----|------|
| Windows | `C:\Users\<user>\.config\wtch\wtch.yml` |
| macOS | `~/.config/wtch/wtch.yml` |
| Linux | `~/.config/wtch/wtch.yml` |

Add presets or custom watches to get started:

```yaml
watch:
  - preset: github
  - preset: openai
  - preset: cloudflare
```

See [docs/configuration.md](docs/configuration.md) for the full reference.
