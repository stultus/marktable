# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Common commands

Frontend (run from repo root):
- `pnpm install` — install JS deps
- `pnpm dev` — Vite/SvelteKit dev server only (frontend without Tauri shell)
- `pnpm build` — produce static `build/` for Tauri to bundle
- `pnpm check` — `svelte-check` type / template lint

Backend + app (also from repo root, via the `tauri` script):
- `pnpm tauri dev` — run the desktop app in dev mode (spawns Vite + Rust)
- `pnpm tauri build` — produce a release bundle (`.dmg` / `.msi` / `.AppImage`)

Rust core (from `src-tauri/`):
- `cargo build --lib` — fast lib-only build (skips Tauri context generation)
- `cargo test` — run unit + integration tests (`tests/round_trip.rs`)
- `cargo test <name>` — run a single test by name substring

## Product

**MarkTable Editor** — a cross-platform desktop app (Tauri + Rust + Svelte) that opens a folder of structured-content files (or a single structured-data file) as a spreadsheet-like table and writes edits back to the original files via a single Save All. See `prd.md` for the full spec.

The mapping is direct:

| Source | Row | Column |
|---|---|---|
| Folder of `.md` files | One file per row | Filename + one frontmatter field per column |
| Single `.json` array file | One object per row | One key per column |
| Single `.yaml` / `.yml` list file | One item per row | One key per column |

Workflow is **Read → Edit → Save All**. Edits are held in memory until Save All; nothing is written before then.

## Architecture

Two processes, three logical layers. Most changes need to respect all three:

1. **Rust core (Tauri backend)** — owns parsers/serializers per format (`.md` + YAML frontmatter, JSON array, YAML list), all file I/O, schema inference, and the canonical in-memory table model with dirty tracking. Each format has a read path that produces a uniform record list and a write path that round-trips back to source preserving structure MarkTable did not touch. **The hard invariants below all live here.**
2. **Tauri IPC boundary** — typed commands (`open_folder`, `open_file`, `save_all`, `add_row`, `delete_row`, `add_column`, `delete_column`, etc.) and events for progress / per-file errors. The frontend never touches the filesystem directly; everything goes through commands.
3. **Svelte frontend (webview)** — renders cells type-aware (toggle, chip/tag, date, etc.), surfaces dirty state, and dispatches commands. Built with SvelteKit (adapter-static, SPA mode) + Vite + TypeScript. The frontend currently *does* hold the editing state during a session (mutates a `TableModel` clone in `src/lib/TableView.svelte`) and sends the whole model back to `save_all`. This is intentional simplicity for the MVP; if it gets bigger we'll move dirty tracking into the Rust core.

**Layout pointers:**
- `src-tauri/src/model/` — `Value`, `FieldType`, `Record`, `Schema`, `TableModel`. Key invariant: `Record.fields` is an `IndexMap` so original frontmatter/key order is preserved.
- `src-tauri/src/formats/` — one module per format. The `markdown` module keeps the body verbatim by capturing the byte offset where the body starts during `parse(...)`.
- `src-tauri/src/commands.rs` — `open_folder`, `open_file`, `save_all`. These are pure I/O wrappers around the model + formats layers.
- `src/lib/api.ts` — the typed contract between Rust commands and the Svelte UI (mirrors the Rust `serde` shapes — keep them in sync).
- `src/lib/StartScreen.svelte`, `src/lib/TableView.svelte` — the only two screens. `+page.svelte` toggles between them.

**Library notes / known risks:**
- **YAML round-trip with comment preservation is NOT YET implemented.** Current code uses `serde_yaml` for both parse and serialize, which drops all comments. `formats::yaml::parse` detects the presence of comments via a heuristic and the `open_file` command surfaces a warning on YAML files that contain them. The PRD-mandated comment-preservation behavior is the most important post-MVP fix. Plan: span-aware rewrite (emit original text for unchanged items, re-serialize only changed items, append new fields after the last existing field per item).
- **JSON indentation style** is detected from the input (tab / 2 / 4 spaces) in `formats::json::Indent::detect` and reproduced via `serde_json::ser::PrettyFormatter`.
- **Markdown frontmatter:** the parser captures the byte offset of the body start and never re-serializes the body — `formats::markdown::parse` returns `body: String` taken straight from the original text.
- **`indexmap` everywhere** to preserve key order in records.

## Invariants that aren't obvious from the code

These are the rules a future implementation must not break — they're easy to violate accidentally during refactors:

- **Null preservation.** Every schema key is written to every record on Save All, even when empty. YAML writes `key:` (no value); JSON writes `"key": null`. Keys are never omitted. New columns propagate to *every* existing record on the next Save All, not just new rows.
- **Markdown body is sacred.** Only the frontmatter block between `---` delimiters is rewritten. Body content after the closing `---` is never touched.
- **YAML comments stay put.** MarkTable never adds, removes, or moves comment lines. New fields are appended after the last existing field in each record's block.
- **Original field order is preserved per file.** New fields append at the end of the record; existing fields keep their position.
- **No writes before Save All.** Edits live in memory only. Files are untouched until the user explicitly triggers Save All.
- **Save All ordering.** Data writes happen first, then file renames. A failed data write is reported per file and does not stop other files. A failed rename after a successful data write surfaces a retry error — the data write is **not** rolled back.
- **Unparseable files** show an error indicator on the row and are excluded from editing rather than blocking the table.
- **Filename column** exists only in folder mode, is always first, and editing it queues a rename (not an immediate one).

## MVP scope boundaries

The PRD's §6.2 "Out of MVP" list (XML, TOML, sort/filter/hide/reorder, named views, CSV, cross-format export, schema reuse, per-column type override, Quick Add) is a hard line for the MVP — don't pull these forward without an explicit decision. Roadmap is in §10.

## Non-functional bars

- Open a folder of 500 files in <3s on a standard dev machine.
- Zero network requests, ever. Fully local.
- Fields and formatting outside changed values are not altered.
