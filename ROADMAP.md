# Roadmap

This is a solo project. Progress depends on available time. Contributions and feedback are welcome via [issues](https://github.com/ysaunier/wtch/issues).

## In progress

- [ ] Improved ordering system with group-level reordering, drag-and-drop, and collapse/expand
- [ ] Preset validation: test each of the 375+ presets against live status pages (large effort, done incrementally)
- [ ] Code signing for Windows and macOS (pending SignPath Foundation approval)

## Ideas (no timeline)

- [ ] Auto-update mechanism
- [ ] Notification history panel
- [ ] Custom notification sounds
- [ ] Import/export config
- [ ] Preset catalog browser with search and categories
- [ ] Tray icon tooltip with summary status

## Done

- [x] Cross-platform builds (Windows, macOS ARM/Intel, Linux)
- [x] CI/CD with GitHub Actions (test + release)
- [x] 375+ built-in service presets, based on [stts](https://github.com/inket/stts)
- [x] 4 adapters: StatusPage, JSON, HTML, Google
- [x] Google adapter with per-product and per-region filtering
- [x] Snowflake presets split by cloud provider and region (51 regions)
- [x] GCP presets split by location (40 regions)
- [x] 1Password presets split by region (USA, Canada, Europe, Enterprise)
- [x] Individual Google Workspace presets (Gmail, Drive, Meet, etc.)
- [x] Groups to organize watches with section headers
- [x] Expandable detail panels with dynamic components
- [x] Progress bar support in dynamic details
- [x] `link` field to separate display URL from data URL
- [x] Array index support in templates
- [x] WSL, PowerShell, and bash built-in shells
- [x] Debug logging to file with UI toggle
- [x] Settings panel with preset search, theme picker
- [x] About page with version and links
- [x] Light and dark themes
- [x] Desktop notifications on status changes
- [x] Hidden console windows on Windows (no cmd/powershell flash)
- [x] Script execution opt-in (`allow_scripts`, disabled by default)
- [x] Conventional commits lint in CI
- [x] Auto-sync version from git tag in release CI
