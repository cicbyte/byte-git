# byte-git

**中文**: [README.md](README.md)

A Git power toolkit (CLI + desktop): branch hygiene, named stashes, multi-repo dev stats, full backups, license & gitignore template management — implemented in Rust with the workspace layout of [byte-writer](https://github.com/cicbyte/byte-writer) (`crates/core` + `apps/cli` + `apps/desktop` Tauri).

> The feature spec is ported from [git-mate](https://github.com/cicbyte/git-mate) (Go + Wails) with identical behavior; the desktop app reuses its Vue 3 frontend via an adapter layer running on Tauri v2.

## Feature Matrix

| Command | Description | Status |
|---|---|---|
| `version` | Version info | ✅ |
| `config` | Global config (YAML) get/set/list | ✅ |
| `branch` | Clean merged branches, list orphans | ✅ |
| `stash` | Named stash save/pop/list with search | ✅ |
| `info` | Repo summary / health check | ✅ |
| `stats` | Multi-repo commit stats, activity heatmap, streaks, languages | ✅ |
| `backup` | Full repo backup (git clone --mirror) | ✅ |
| `license` | 47 OSS license templates, create & detect | ✅ |
| `ignore` | .gitignore template management (idempotent append) | ✅ |
| `clone` | Proxy-accelerated clone (Lua rules engine) | ✅ |
| `download` | GitHub Release asset download | ✅ |
| `archive` | Repository snapshot archiving | ✅ |
| `ai` | AI commit message / AI rewrite | ✅ |
| `rewrite` | Commit history metadata rewriting | ✅ |
| `review` | AI code review (external OCR integration) | ✅ |
| `secret` | Secret leak scanning (external gitleaks) | ✅ |
| `hook` | Git hook management (Lua scripts) | ✅ |
| `rebase` | Rebase preview & auto-squash | ✅ |
| `undo` | Safe undo | ✅ |
| `logview` | Git log TUI browser | ✅ |

## Install & Build

Requires Rust >= 1.85, Git >= 2.0.

```bash
git clone https://github.com/cicbyte/byte-git
cd byte-git
cargo build --release -p byte-git-cli
# Binary: target/release/byte-git
```

Desktop (Tauri v2, needs Node >= 20):

```bash
cd apps/desktop
npm --prefix frontend install
cargo tauri build          # or: cargo tauri dev
```

## Quick Start

```bash
byte-git version                     # Show version
byte-git info summary                # Repo overview
byte-git info health                 # Health check
byte-git branch clean --dry-run      # Preview merged branches to clean
byte-git stash save --name wip       # Named save
byte-git stash list --search wip     # Search
byte-git stats --days 30             # Dev stats of last 30 days
byte-git backup create               # Mirror-backup current repo
byte-git license create mit          # Create MIT LICENSE
byte-git ignore add rust node        # Append rust/node gitignore templates
```

## Project Structure

```
byte-git/
├── apps/
│   ├── cli/                 # CLI (clap command tree, binary: byte-git)
│   └── desktop/             # Tauri v2 desktop app
│       ├── frontend/        # Vue 3 + Vite + Pinia (with Wails→Tauri adapter)
│       └── src-tauri/       # Tauri command layer (thin wrapper over crates/core)
├── crates/
│   └── core/                # Domain core: git ops / config / backup / stats / templates
│       └── assets/          # Embedded assets: 47 licenses + gitignore templates + AI prompts
├── docs/                    # Command docs
├── scripts/                 # Release scripts
└── rules/ prompts/          # Lua proxy rules / prompt sources
```

## Config & Data

```
~/.cicbyte/byte-git/
  config/config.yaml    # Global config (AI model, GitHub token, feature toggles)
  backups/              # Full repo backups
  prompts/              # AI prompt templates (customizable)
~/.cicbyte/data/        # Cross-project shared data (cicbyte tool family)
.git/byte-git/          # Per-repo data (hook config, rewrite JSON, AI cache)
```

## Desktop Architecture

The git-mate Vue 3 frontend calls Wails bindings via `window.go.main.ByteGitApp.Method(...)`.
byte-git ships a same-name adapter under `frontend/src/wailsjs/`:

- `go/main/ByteGitApp.ts` — Proxy bridge: camelCase method → snake_case Tauri command, arguments marshalled via a signature table
- `runtime/runtime.ts` — `EventsOn/EventsOff/EventsEmit` → Tauri events

The frontend runs on Tauri unchanged; the Rust command set grows as core modules land.

## License

[MIT](LICENSE) © 2025 cicbyte
