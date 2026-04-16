<p align="center">
  <img src="src-tauri/icons/128x128.png" alt="wtch" width="80" />
</p>

<h1 align="center">wtch</h1>

<p align="center">A lightweight system tray app for monitoring the status of cloud services, built with <a href="https://tauri.app/">Tauri</a> and <a href="https://vuejs.org/">Vue</a>.</p>

<p align="center">
  <img src="https://img.shields.io/badge/Rust-1.70+-orange?logo=rust" alt="Rust" />
  <img src="https://img.shields.io/badge/Vue-3-42b883?logo=vuedotjs" alt="Vue 3" />
  <img src="https://img.shields.io/badge/Tauri-2-24c8db?logo=tauri" alt="Tauri 2" />
  <img src="https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-blue" alt="Platform" />
  <img src="https://img.shields.io/badge/License-MIT-yellow" alt="MIT License" />
</p>

![wtch screenshot](docs/images/wtch.png)

Click the tray icon to see the status of your services at a glance. Expand any service to view individual component health. Get notified when something goes down.

## Features

- System tray app with popup panel
- 375+ built-in service presets (GitHub, AWS, GCP, OpenAI, Snowflake, ...)
- Expandable details per service (components, regions, sub-services)
- Desktop notifications on status changes
- Light and dark themes
- Custom watches via YAML config (HTTP, script, adapter)

## Getting started

Download from [Releases](https://github.com/ysaunier/wtch/releases) or [build from source](INSTALL.md).

See [docs/configuration.md](docs/configuration.md) for config examples and adapter details.

## Contributing

Found a bug? Missing a service preset? [Open an issue](https://github.com/ysaunier/wtch/issues).

## Acknowledgements

Inspired by [stts](https://github.com/inket/stts) by [@inket](https://github.com/inket). Thank you for the inspiration.

## Author

[Yoann Saunier](https://ysaunier.dev/) - [@ysaunier](https://github.com/ysaunier)

<a href="https://buymeacoffee.com/ysaunier" target="_blank"><img src="https://cdn.buymeacoffee.com/buttons/v2/default-yellow.png" alt="Buy Me A Coffee" height="48" width="173"></a>

## License

[MIT](LICENSE) - Yoann Saunier | [Privacy Policy](PRIVACY.md) | [Install](INSTALL.md)
