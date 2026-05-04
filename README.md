# MarkTable

A cross-platform desktop app that opens a folder of structured-content files (or a single structured-data file) as a spreadsheet-like table and writes edits back to the original files via a single **Save All**.

Built with **Tauri** + **Rust** + **SvelteKit**. Fully local — zero network requests, ever.

## What it does

| Source | Row | Column |
|---|---|---|
| Folder of `.md` files | One file per row | Filename + one frontmatter field per column |
| Single `.json` array file | One object per row | One key per column |
| Single `.yaml` / `.yml` list file | One item per row | One key per column |

Workflow is **Read → Edit → Save All**. Edits are held in memory until you save; nothing is written before then.

## Why

Editing structured content one file at a time is tedious. MarkTable lets you scan and edit a whole content set the way you'd edit a spreadsheet, while preserving everything the table doesn't touch — original key order, markdown bodies, JSON indentation style, and (post-MVP) YAML comments.

## Getting started

Prerequisites: [Rust](https://www.rust-lang.org/tools/install), [pnpm](https://pnpm.io/installation), and the [Tauri prerequisites](https://tauri.app/start/prerequisites/) for your platform.

```sh
pnpm install
pnpm tauri dev      # run the desktop app in dev mode
```

To produce a release bundle (`.dmg` / `.msi` / `.AppImage`):

```sh
pnpm tauri build
```

## Project layout

```
src/                 SvelteKit frontend (StartScreen, TableView, IPC client)
src-tauri/src/
  model/             Value, FieldType, Record, Schema, TableModel
  formats/           Per-format parsers/serializers (markdown, json, yaml)
  commands.rs        Tauri IPC: open_folder, open_file, save_all, …
src-tauri/tests/     Round-trip tests
prd.md               Full product spec
CLAUDE.md            Architecture notes & invariants
```

## Common commands

Frontend (run from repo root):

- `pnpm dev` — Vite/SvelteKit dev server (frontend without Tauri shell)
- `pnpm build` — produce static `build/` for Tauri to bundle
- `pnpm check` — `svelte-check` type / template lint

App (also from repo root):

- `pnpm tauri dev` — run the desktop app in dev mode (Vite + Rust)
- `pnpm tauri build` — produce a release bundle

Rust core (from `src-tauri/`):

- `cargo build --lib` — fast lib-only build (skips Tauri context generation)
- `cargo test` — run unit + integration tests

## Invariants

The implementation guarantees these properties on every Save All:

- **Null preservation.** Every schema key is written to every record, even when empty. Keys are never omitted.
- **Markdown body is sacred.** Only the frontmatter block between `---` delimiters is rewritten.
- **Original field order is preserved per file.** New fields append at the end.
- **No writes before Save All.** Edits live in memory only.
- **Save All ordering.** Data writes happen first, then file renames. A failed write on one file doesn't stop the rest.

## Status

MVP. YAML comment-preservation on round-trip is the most important post-MVP fix — the parser flags files containing comments and the UI surfaces a warning. See [`prd.md`](./prd.md) for the full spec and roadmap.

## License

MIT
