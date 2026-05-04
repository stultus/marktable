# MarkTable Editor — Product Requirements Document

**Version:** 0.4 (MVP)
**Status:** Draft
**Author:** Hiran V
**Last Updated:** May 2026

---

## 1. Overview

MarkTable Editor is a cross-platform desktop application (built with Tauri, Rust, and Svelte) that lets users open a folder of structured content files — or a single structured data file — and edit them in a spreadsheet-like table view. It writes changes back to the original files in their original format via a single Save All action.

The name reflects its dual purpose: it handles **markup** formats (Markdown with frontmatter) and **markup languages** (JSON, YAML) through a unified table interface.

It is format-agnostic, SSG-agnostic, and fully local. No server, no authentication, no internet required.

---

## 2. Problem Statement

Users of static site generators and structured content systems manage data across dozens or hundreds of files. Editing frontmatter properties — titles, dates, tags, status flags, URLs, categories — requires opening each file individually. No existing tool provides a spreadsheet-like bulk editing experience that works natively within the developer's existing environment and writes back to the original file format.

For single structured files (JSON arrays, YAML lists), the same problem applies: the only option is editing raw text, with no tabular view to navigate or modify records efficiently.

---

## 3. Target Users

**Primary:** Developers and technical content creators who use static site generators (Jekyll, Hugo, Astro, Eleventy, Gatsby, Hexo) and manage content as files in a repository.

**Secondary:** Anyone who maintains structured data as JSON or YAML files — bookmark lists, reference directories, product catalogs, event listings, reading logs, and similar collections.

---

## 4. Core Concept

A file or folder maps directly to a table:

| Source | Row | Column |
|---|---|---|
| Folder of `.md` files | One file per row | Filename + one frontmatter field per column |
| Single `.json` array file | One object per row | One key per column |
| Single `.yaml` list file | One item per row | One key per column |

The user's workflow is linear:

**Read → Edit → Save All**

- **Read** — MarkTable parses the folder or file and renders all records as a table
- **Edit** — user edits cells inline; changes are held in memory and marked visually
- **Save All** — all dirty records are written back to their source files at once

---

## 5. Use Cases

### 5.1 Bulk frontmatter editing in a Jekyll/Hugo/Astro collection

A user has a `_books/` folder with 80 markdown files, each representing a book with frontmatter fields like `title`, `author`, `genre`, `rating`, `read`, `year`. They open the folder in MarkTable (via File → Open Folder, or by dragging the folder onto the app), see all 80 books as rows, edit ratings and read status in bulk, rename a few files, and hit Save All. Each individual `.md` file is updated.

### 5.2 Editing a single structured data file

A user maintains a YAML list file powering a directory page on their site — each record has fields like `name`, `city`, `state`, `active`, `categories`, `tags`, `url`. They open it in MarkTable, toggle `active` flags, fix URLs, and hit Save All. The file is written back as valid YAML with all original comments preserved in place.

### 5.3 Managing a `/uses` or `/bookmarks` page

A user has a `data/bookmarks.json` array powering their site's links page. They open it in MarkTable, add new entries as rows, fill in fields, and Save All. New records are appended to the JSON array.

### 5.4 Generic structured data editing without an SSG

A user has no SSG — just a folder of `.md` files used as a personal knowledge base, or a JSON file used as a data source for a script. MarkTable works the same way regardless.

---

## 6. MVP Scope

### 6.1 In MVP

- Open a folder of `.md` files as a table
- Open a single `.json` array file as a table
- Open a single `.yaml` / `.yml` list file as a table
- First column is filename in folder mode — editable; renaming it renames the file on Save All
- Inline cell editing with field-type-aware cell rendering
- Read → Edit → Save All workflow
- Add a new row (creates a new file or appends a new record)
- Delete a row (deletes the file or removes the record)
- Add a new column — adds the key to every row with an empty value, keeping schema consistent across all records
- Delete a column — removes the key from every file including those with empty or absent values
- Auto-infer schema and field types on open
- On open, show an info-level warning listing any columns where 100% of values are empty — non-blocking, user can dismiss and proceed
- Null preservation — every key in the schema is written to every record on Save All, even if the value is empty; empty cell writes as an empty key, never omitted
- Cell rendering: scalars as typed cells, arrays as tag/chip cells, complex nested values as raw editable strings
- YAML comment preservation — comments stay exactly where they are; MarkTable never adds, removes, or moves comment lines

### 6.2 Out of MVP

- XML support
- TOML frontmatter support
- Sort, filter, hide, reorder columns
- Named views
- CSV import / export
- Cross-format export
- Schema save and reuse
- Field type override per column
- Quick Add / single record form view

---

## 7. Functional Requirements

### 7.1 Entry Points

- **File → Open Folder…** — pick a folder of `.md` files
- **File → Open File…** — pick a `.json`, `.yaml`, or `.yml` file
- **Drag and drop** a folder or supported file onto the app window
- **OS file association** — open a `.json` / `.yaml` / `.yml` file with MarkTable from the system file manager
- **Recent items** list on the start screen for quick reopening

### 7.2 Schema Inference

On open, MarkTable reads all files/records and builds the column set:

- All unique field names across all rows become columns
- Field type inferred per column from the majority of non-empty values:
  - `true` / `false` → boolean
  - ISO date strings → date
  - Pure numeric values → number
  - Arrays → list
  - Everything else → text
- Complex nested objects that cannot be cleanly typed are treated as raw strings
- Fields present in some rows but not others are included as columns; missing values are empty cells

### 7.3 Empty Column Warning

After open, before the user begins editing, MarkTable checks for columns where every value across all records is empty or absent. If any are found, an info-level notice lists them. It is dismissable and non-blocking — no action is required, the user can proceed to edit normally.

### 7.4 Null Preservation

Every key in the table schema is written to every record on Save All — regardless of whether the cell has a value. An empty cell writes as:

- YAML: `rating:` (key with no value)
- JSON: `"rating": null`

Keys are never omitted on write. This keeps schema consistent for SSG templates and any other consumers that expect the key to exist.

When a new column is added, it is immediately written to every existing record as an empty key on the next Save All — not just to new records.

### 7.5 Filename Column

- Always the first column in folder mode
- Shows the filename of the source `.md` file without path
- Editable — changing the value queues a file rename for Save All
- Not present in single-file mode

### 7.6 Cell Rendering

- **Text** — plain inline text input
- **Number** — numeric input
- **Boolean** — toggle / checkbox
- **Date** — text input with date format hint
- **List / Array** — tag/chip style input; tags are addable and removable inline
- **Raw string** — plain text input for complex or nested values; read and written as-is

### 7.7 Editing

- Click a cell to edit inline
- Tab / Shift+Tab to move between cells
- Enter to confirm, Escape to cancel
- All changes held in memory until Save All
- Dirty cells and rows are visually indicated

### 7.8 Save All Behaviour

Save All executes in this order:

1. Write data changes to all dirty files
2. Execute file renames for any edited filename column values

If a data write fails, the user is notified per file and Save All continues with remaining files.

If a file rename fails after the data write has succeeded, a retry error is shown for the rename only — the data save is not rolled back.

Original field order is preserved per file. New fields are appended at the end of each record's block.

### 7.9 Row Operations

- **Add row:** Appends a new empty row. In folder mode, prompts for a filename. In file mode, appends a new empty object. All schema keys are written with empty values on Save All.
- **Delete row:** Marks row for deletion. On Save All, deletes the `.md` file or removes the record. Requires confirmation.

### 7.10 Column Operations

- **Add column:** Prompts for field name and type. On Save All, the key is written to every record — existing and new — with an empty value where not filled.
- **Delete column:** Removes the key from every file in the collection, including files where the value is empty or absent. Hard delete of the key, not a value clear. Requires confirmation.

### 7.11 Format Handling

**Markdown with YAML frontmatter:**
- Parse frontmatter between `---` delimiters
- Body content after the closing `---` is never modified
- Only the frontmatter block is rewritten on Save All

**JSON:**
- Must be a top-level array of objects
- Written back as formatted JSON preserving detected indentation style

**YAML:**
- Must be a top-level list of mappings
- Comments preserved exactly in place
- New fields appended after the last existing field in each record's block
- Written back with 2-space indentation as default

### 7.12 Error Handling

- Unparseable files show an error indicator on the row and are excluded from editing
- Save All write failures surface a clear per-file error; remaining files continue to save
- Rename failures after a successful data write surface a retry error; data write is not rolled back

---

## 8. Non-Functional Requirements

- Opens a folder of 500 files within 3 seconds on a standard developer machine
- No network requests at any time
- Files are not modified until Save All is explicitly triggered
- Fields and formatting outside changed values are not altered

---

## 9. Desktop App Specifics

- **Stack:** Tauri (Rust backend + system webview frontend), Svelte (with TypeScript) for the UI, Vite as the frontend build tool
- **Backend (Rust):** owns all file I/O, format parsing, format serialization, schema inference, and the in-memory table model. Exposes typed Tauri commands (`open_folder`, `open_file`, `save_all`, etc.) over IPC
- **Frontend (Svelte):** owns rendering, cell editing, dirty-state UI, and dispatching commands to the backend. Holds no canonical state — the Rust core is the source of truth for parsed records
- **Targets:** macOS (Apple Silicon + Intel), Windows (x64), Linux (x64). Distributed as `.dmg` / `.msi` / `.AppImage` bundles
- **Code signing:** macOS notarized; Windows code-signed
- **Auto-update** is a post-MVP concern; MVP ships as plain installable bundles

---

## 10. Post-MVP Roadmap

**V1.1**
- Sort and filter rows by column value
- Hide / show columns
- Reorder columns by drag
- Named views per folder/file
- Quick Add — single record form view for appending without opening full table

**V1.2**
- XML support
- TOML frontmatter support
- CSV import and export

**V1.3**
- Cross-format export (folder of `.md` ↔ single `.json` / `.yaml`)
- Schema save and reuse
- Field type override per column

**Future**
- RSS feed import → populate table → export to file
- Multi-folder / multi-file view
- Git-aware diff summary before Save All
