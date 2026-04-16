# Contributing

Thanks for your interest in wtch! This is a solo project but contributions are welcome.

## Getting started

See [INSTALL.md](INSTALL.md) for build instructions.

## Branch naming

```
feature/short-description    New feature
fix/short-description        Bug fix
preset/service-name          New or updated preset
docs/short-description       Documentation changes
```

## Commit messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add About page with links to GitHub and website
fix: duplicate tray icon on Windows
docs: update configuration reference
preset: add Datadog preset
refactor: extract shell runner logic
test: add array index template tests
chore: upgrade GitHub Actions to v5
```

### Allowed prefixes

| Prefix | Use for |
|--------|---------|
| `feat` | New feature |
| `fix` | Bug fix |
| `docs` | Documentation only |
| `preset` | New or updated preset |
| `refactor` | Code change that neither fixes nor adds |
| `test` | Adding or updating tests |
| `chore` | Build, CI, tooling, dependencies |
| `style` | Formatting, no code change |

Keep the subject short (under 72 chars), lowercase, no period at the end.

## Pull requests

- One feature or fix per PR
- Squash merge into main (enforced by repo settings)
- Add a short description of what and why
- If adding a preset, include the status page URL

## Adding a preset

1. Create a YAML file in `presets/` named after the service (lowercase, hyphens)
2. Test it locally by adding `- preset: your-preset` to your config
3. Make sure it resolves a correct status (green when service is up)

See existing presets for reference. Most services use the StatusPage adapter:

```yaml
name: My Service
type: adapter
adapter: urn:wtch:adapter:statuspage
url: https://status.myservice.com
interval: 60s
conditions:
  error: '.status.indicator == "major" or .status.indicator == "critical"'
  warning: '.status.indicator == "minor"'
  maintenance: '.status.indicator == "maintenance"'
  success: '.status.indicator == "none"'
display:
  style: status
  description: "{{ .status.description }}"
  details_from: .components
  detail_title: .name
  detail_status: .status
```

## Code style

- Rust: follow `cargo clippy` and `cargo fmt`
- Vue/TypeScript: no linter enforced yet, follow existing patterns
- English only in code and comments
- No dead code, no commented-out code
