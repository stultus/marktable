<script lang="ts">
  import { onMount, untrack, tick } from "svelte";
  import { fly, fade } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import {
    saveAll,
    valueToDisplay,
    displayToValue,
    basename,
    type TableModel,
    type Value,
    type FieldType,
    type Record as MtRecord,
  } from "./api";
  import Select from "./Select.svelte";
  import DatePicker from "./DatePicker.svelte";

  const FIELD_TYPE_OPTIONS: { value: FieldType; label: string; hint: string }[] = [
    { value: "text", label: "Text", hint: "string" },
    { value: "number", label: "Number", hint: "int / float" },
    { value: "boolean", label: "Boolean", hint: "true / false" },
    { value: "date", label: "Date", hint: "yyyy-mm-dd" },
    { value: "list", label: "List", hint: "comma-separated" },
    { value: "raw", label: "Raw", hint: "passthrough" },
  ];

  let {
    table,
    onClose,
  }: {
    table: TableModel;
    onClose: () => void;
  } = $props();

  let model = $state<TableModel>(untrack(() => table));
  let dirtyCells = $state(new Set<string>());
  let invalidCells = $state(new Map<string, string>());
  let schemaDirty = $state(false);
  let saving = $state(false);
  let toast = $state<{ kind: "success" | "error"; text: string } | null>(null);
  let dismissedWarnings = $state(new Set<number>());

  // Add Column modal state
  let addColOpen = $state(false);
  let addColName = $state("");
  let addColType = $state<FieldType>("text");
  let addColError = $state<string | null>(null);

  // Edit Column modal state
  let editColOpen = $state(false);
  let editColOriginalName = $state("");
  let editColName = $state("");
  let editColType = $state<FieldType>("text");
  let editColError = $state<string | null>(null);
  let editColInput = $state<HTMLInputElement | null>(null);

  // Delete-column confirmation
  let pendingDeleteCol = $state<string | null>(null);

  // Add Row modal (folder mode requires a filename; file mode is a one-click append)
  let addRowOpen = $state(false);
  let addRowFilename = $state("");
  let addRowError = $state<string | null>(null);
  // Tracks rows added in this session so add → delete (without saving) is purely local.
  let rowsDirty = $state(false);

  // Per-cell value at focus time, so Escape can revert in-flight edits.
  const cellOriginal = new Map<string, string>();

  function rememberOriginal(rowIdx: number, colName: string, currentValue: string) {
    cellOriginal.set(cellKey(rowIdx, colName), currentValue);
  }

  /** Focus the editor inside a specific cell. Searches text inputs, textareas,
   *  the boolean checkbox, and the date-picker trigger in priority order. */
  function focusCell(rowIdx: number, colName: string) {
    const sel = [
      `[data-cell="${rowIdx}::${cssEscape(colName)}"] textarea`,
      `[data-cell="${rowIdx}::${cssEscape(colName)}"] input[type="text"]`,
      `[data-cell="${rowIdx}::${cssEscape(colName)}"] .dp-trigger`,
      `[data-cell="${rowIdx}::${cssEscape(colName)}"] input[type="checkbox"]`,
    ].join(", ");
    const el = document.querySelector<HTMLElement>(sel);
    el?.focus();
    if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) {
      el.select?.();
    }
  }

  function cssEscape(s: string): string {
    // CSS attribute selectors can't contain bare quotes etc. Escape the few
    // characters our column names might plausibly contain.
    return s.replace(/(["\\])/g, "\\$1");
  }

  function focusNext(rowIdx: number, colName: string) {
    const cols = model.schema.columns;
    const colIdx = cols.findIndex((c) => c.name === colName);
    if (colIdx < 0) return;
    if (colIdx + 1 < cols.length) {
      focusCell(rowIdx, cols[colIdx + 1].name);
    } else if (rowIdx + 1 < model.rows.length) {
      focusCell(rowIdx + 1, cols[0].name);
    }
  }

  function focusPrev(rowIdx: number, colName: string) {
    const cols = model.schema.columns;
    const colIdx = cols.findIndex((c) => c.name === colName);
    if (colIdx < 0) return;
    if (colIdx > 0) {
      focusCell(rowIdx, cols[colIdx - 1].name);
    } else if (rowIdx > 0) {
      focusCell(rowIdx - 1, cols[cols.length - 1].name);
    }
  }

  /** Keydown handler for text-overlay editors (input + textarea).
   *  - Enter (no Shift) → commit + advance one row, same column
   *  - Shift+Enter      → default (newline in textarea)
   *  - Tab / Shift+Tab  → in a textarea, prevent the literal tab insert and
   *                       advance row-major focus instead
   *  - Escape           → revert to the value at focus time, blur (no commit) */
  function cellKeyDown(
    e: KeyboardEvent,
    rowIdx: number,
    colName: string,
    type: FieldType,
  ) {
    const target = e.currentTarget as HTMLInputElement | HTMLTextAreaElement;
    if (e.key === "Escape") {
      e.preventDefault();
      const orig = cellOriginal.get(cellKey(rowIdx, colName));
      if (orig !== undefined) target.value = orig;
      target.blur();
      return;
    }
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      commitEdit(rowIdx, colName, target.value, type);
      if (rowIdx + 1 < model.rows.length) {
        focusCell(rowIdx + 1, colName);
      } else {
        target.blur();
      }
      return;
    }
    if (e.key === "Tab" && target instanceof HTMLTextAreaElement) {
      // Browsers insert "\t" into a textarea on Tab. Override so it advances
      // focus like every other input does by default.
      e.preventDefault();
      commitEdit(rowIdx, colName, target.value, type);
      if (e.shiftKey) focusPrev(rowIdx, colName);
      else focusNext(rowIdx, colName);
    }
  }

  const cellKey = (row: number, col: string) => `${row}::${col}`;

  let addColInput = $state<HTMLInputElement | null>(null);
  let addRowInput = $state<HTMLInputElement | null>(null);

  function openAddColumn() {
    addColName = "";
    addColType = "text";
    addColError = null;
    addColOpen = true;
    tick().then(() => addColInput?.focus());
  }
  function closeAddColumn() {
    addColOpen = false;
  }
  function confirmAddColumn() {
    const name = addColName.trim();
    if (!name) {
      addColError = "Field name is required.";
      return;
    }
    if (model.schema.columns.some((c) => c.name === name)) {
      addColError = `A column named "${name}" already exists.`;
      return;
    }
    model.schema.columns = [
      ...model.schema.columns,
      { name, type: addColType },
    ];
    // Schema-level change → every row's frontmatter is rewritten on save.
    for (const r of model.rows) r.dirty = true;
    schemaDirty = true;
    addColOpen = false;
  }

  function newRowId(): string {
    return `new-${Date.now().toString(36)}-${Math.floor(Math.random() * 1e6).toString(36)}`;
  }

  function emptyRecord(): MtRecord {
    const fields: { [k: string]: Value } = {};
    for (const c of model.schema.columns) fields[c.name] = { kind: "null" };
    return { fields };
  }

  function addInlineRow() {
    if (model.mode.kind === "folder") {
      addRowFilename = "";
      addRowError = null;
      addRowOpen = true;
      tick().then(() => addRowInput?.focus());
      return;
    }
    const nextIndex = model.rows.length;
    const newRow = {
      id: newRowId(),
      source: { kind: "inline" as const, index: nextIndex },
      record: emptyRecord(),
      parse_error: null,
      pending_delete: false,
      dirty: true,
    };
    model.rows = [...model.rows, newRow];
    rowsDirty = true;
  }

  function joinPath(folder: string, name: string): string {
    const sep = folder.includes("\\") && !folder.includes("/") ? "\\" : "/";
    const base = folder.endsWith("/") || folder.endsWith("\\")
      ? folder.slice(0, -1)
      : folder;
    return `${base}${sep}${name}`;
  }

  function confirmAddFolderRow() {
    let name = addRowFilename.trim();
    if (!name) {
      addRowError = "Filename is required.";
      return;
    }
    if (!/\.md$/i.test(name)) name = `${name}.md`;
    if (/[\\/]/.test(name.slice(0, -3))) {
      addRowError = "Filename cannot contain path separators.";
      return;
    }
    if (model.mode.kind !== "folder") return;
    const fullPath = joinPath(model.mode.path, name);
    if (model.rows.some((r) => r.source.kind === "file" && r.source.path === fullPath)) {
      addRowError = `A row for "${name}" already exists.`;
      return;
    }
    model.rows.push({
      id: newRowId(),
      source: { kind: "file", path: fullPath, original_text: "" },
      record: emptyRecord(),
      parse_error: null,
      pending_delete: false,
      dirty: true,
    });
    rowsDirty = true;
    addRowOpen = false;
  }

  function toggleDeleteRow(rowIdx: number) {
    const r = model.rows[rowIdx];
    r.pending_delete = !r.pending_delete;
    rowsDirty = true;
  }

  function openEditColumn(col: { name: string; type: FieldType }) {
    editColOriginalName = col.name;
    editColName = col.name;
    editColType = col.type;
    editColError = null;
    editColOpen = true;
    tick().then(() => editColInput?.focus());
  }
  function cancelEditColumn() {
    editColOpen = false;
  }
  function confirmEditColumn() {
    const name = editColName.trim();
    if (!name) {
      editColError = "Field name is required.";
      return;
    }
    if (
      name !== editColOriginalName &&
      model.schema.columns.some((c) => c.name === name)
    ) {
      editColError = `A column named "${name}" already exists.`;
      return;
    }
    const oldName = editColOriginalName;
    // Replace the column metadata in-place so order is preserved.
    model.schema.columns = model.schema.columns.map((c) =>
      c.name === oldName ? { name, type: editColType } : c
    );
    if (name !== oldName) {
      // Rename the field key in every row's record while preserving order.
      for (const row of model.rows) {
        const next: { [k: string]: Value } = {};
        for (const [k, v] of Object.entries(row.record.fields)) {
          next[k === oldName ? name : k] = v;
        }
        row.record.fields = next;
        row.dirty = true;
      }
      // Migrate dirty / invalid cell keys from "rowIdx::oldName" → "rowIdx::name".
      const remap = (k: string) =>
        k.endsWith(`::${oldName}`)
          ? k.slice(0, -oldName.length) + name
          : k;
      const nextDirty = new Set<string>();
      for (const k of dirtyCells) nextDirty.add(remap(k));
      dirtyCells = nextDirty;
      const nextInvalid = new Map<string, string>();
      for (const [k, msg] of invalidCells) nextInvalid.set(remap(k), msg);
      invalidCells = nextInvalid;
    }
    schemaDirty = true;
    editColOpen = false;
  }

  function requestDeleteColumn(name: string) {
    pendingDeleteCol = name;
  }
  function cancelDeleteColumn() {
    pendingDeleteCol = null;
  }
  function confirmDeleteColumn() {
    const name = pendingDeleteCol;
    if (!name) return;
    model.schema.columns = model.schema.columns.filter((c) => c.name !== name);
    for (const row of model.rows) {
      if (name in row.record.fields) {
        delete row.record.fields[name];
        row.dirty = true;
      }
    }
    // Clear any per-cell dirty flags for this column.
    const next = new Set<string>();
    for (const k of dirtyCells) {
      if (!k.endsWith(`::${name}`)) next.add(k);
    }
    dirtyCells = next;
    // Clear any invalid markers for this column.
    const nextInvalid = new Map<string, string>();
    for (const [k, msg] of invalidCells) {
      if (!k.endsWith(`::${name}`)) nextInvalid.set(k, msg);
    }
    invalidCells = nextInvalid;
    schemaDirty = true;
    pendingDeleteCol = null;
  }

  function validationError(raw: string, type: FieldType): string | null {
    const text = raw.trim();
    if (text === "") return null; // empty is always valid (null cell)
    switch (type) {
      case "number":
        return Number.isFinite(Number(text)) ? null : `"${text}" is not a number`;
      case "boolean":
        return /^(true|false)$/i.test(text) ? null : `"${text}" must be true or false`;
      case "date":
        // ISO yyyy-mm-dd. Lenient — full ISO timestamp also passes.
        return /^\d{4}-\d{2}-\d{2}(T.*)?$/.test(text) ? null : `"${text}" must be YYYY-MM-DD`;
      case "list":
      case "text":
      case "raw":
      default:
        return null;
    }
  }

  function commitEdit(row: number, col: string, raw: string, type: FieldType) {
    const next = displayToValue(raw, type);
    const current = model.rows[row].record.fields[col] ?? ({ kind: "null" } as Value);
    const key = cellKey(row, col);
    const err = validationError(raw, type);
    if (err) {
      invalidCells.set(key, err);
    } else {
      invalidCells.delete(key);
    }
    invalidCells = new Map(invalidCells);
    if (JSON.stringify(next) === JSON.stringify(current)) return;
    model.rows[row].record.fields[col] = next;
    model.rows[row].dirty = true;
    dirtyCells.add(key);
    dirtyCells = new Set(dirtyCells);
  }

  function cellText(rowIdx: number, colName: string): string {
    const v = model.rows[rowIdx].record.fields[colName] ?? ({ kind: "null" } as Value);
    return valueToDisplay(v);
  }

  function fieldVal(rowIdx: number, colName: string): Value {
    return model.rows[rowIdx].record.fields[colName] ?? ({ kind: "null" } as Value);
  }

  function listItems(v: Value): string[] {
    if (v.kind === "list") return v.value.map(valueToDisplay).filter(Boolean);
    return [];
  }

  // Deterministic tag color: hash → palette index.
  const tagPalette = [
    "blue",
    "green",
    "yellow",
    "orange",
    "purple",
    "pink",
    "red",
    "gray",
  ] as const;
  function tagColor(s: string): (typeof tagPalette)[number] {
    let h = 0;
    for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) | 0;
    return tagPalette[Math.abs(h) % tagPalette.length];
  }

  async function doSave() {
    if (invalidCells.size > 0) {
      const n = invalidCells.size;
      toast = {
        kind: "error",
        text: `${n} cell${n === 1 ? "" : "s"} fail${n === 1 ? "s" : ""} type validation — fix highlighted cells before saving.`,
      };
      return;
    }
    saving = true;
    toast = null;
    try {
      const result = await saveAll(model);
      const wrote = result.written.length;
      const failed = result.failures.length;
      if (failed === 0) {
        toast = {
          kind: "success",
          text: `Saved ${wrote} file${wrote === 1 ? "" : "s"}.`,
        };
        dirtyCells = new Set();
        schemaDirty = false;
        // Drop rows that were marked pending_delete and are now gone on disk.
        model.rows = model.rows.filter((r) => !r.pending_delete);
        // Renumber inline rows so labels stay sequential.
        model.rows.forEach((r, i) => {
          if (r.source.kind === "inline") r.source = { kind: "inline", index: i };
        });
        // Clear per-row dirty flags now that on-disk state matches.
        for (const r of model.rows) r.dirty = false;
        rowsDirty = false;
        // Auto-dismiss success after 3.2s
        setTimeout(() => {
          if (toast?.kind === "success") toast = null;
        }, 3200);
      } else {
        toast = {
          kind: "error",
          text: `Saved ${wrote}. ${failed} failed: ${result.failures
            .map((f) => `${basename(f.path)} (${f.message})`)
            .join("; ")}`,
        };
      }
    } catch (e) {
      toast = { kind: "error", text: `Save failed: ${String(e)}` };
    } finally {
      saving = false;
    }
  }

  onMount(() => {
    const onKey = (e: KeyboardEvent) => {
      // Cmd/Ctrl-S → Save All
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "s") {
        e.preventDefault();
        if (!saving && hasUnsaved) doSave();
        return;
      }
      // Escape closes any open modal
      if (e.key === "Escape") {
        if (addColOpen) addColOpen = false;
        else if (editColOpen) editColOpen = false;
        else if (addRowOpen) addRowOpen = false;
        else if (pendingDeleteCol !== null) pendingDeleteCol = null;
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  function rowLabel(rowIdx: number): string {
    const r = model.rows[rowIdx];
    if (r.source.kind === "file") return basename(r.source.path);
    return `${r.source.index + 1}`;
  }

  function modePathSegments(): { icon: "folder" | "file"; segs: string[] } {
    let path: string;
    let icon: "folder" | "file";
    if (model.mode.kind === "folder") {
      path = model.mode.path;
      icon = "folder";
    } else {
      path = model.mode.path;
      icon = "file";
    }
    // Split on / or \, drop empty segments
    const parts = path.split(/[/\\]/).filter(Boolean);
    // Show last 4 segments max
    const segs = parts.slice(Math.max(0, parts.length - 4));
    return { icon, segs };
  }

  let pathInfo = $derived(modePathSegments());
  let dirtyCount = $derived(dirtyCells.size);
  let pendingDeleteCount = $derived(model.rows.filter((r) => r.pending_delete).length);
  let hasUnsaved = $derived(dirtyCount > 0 || schemaDirty || rowsDirty);
  let hasColumns = $derived(model.schema.columns.length > 0);
  let visibleWarnings = $derived(
    model.warnings
      .map((w, i) => ({ w, i }))
      .filter(({ i }) => !dismissedWarnings.has(i)),
  );

  function isEmpty(v: Value): boolean {
    return (
      v.kind === "null" ||
      (v.kind === "string" && v.value === "") ||
      (v.kind === "raw" && v.value === "")
    );
  }
</script>

<div class="view">
  <!-- Topbar -->
  <header class="topbar">
    <button class="icon-btn" onclick={onClose} aria-label="Close">
      <svg viewBox="0 0 16 16" width="14" height="14" fill="none">
        <path d="M10 3L5 8l5 5" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" />
      </svg>
    </button>
    <div class="path" title={model.mode.kind === "folder" ? model.mode.path : (model.mode as any).path}>
      <span class="path-icon" aria-hidden="true">
        {#if pathInfo.icon === "folder"}
          <svg viewBox="0 0 16 16" width="14" height="14" fill="none">
            <path
              d="M1.5 4.4c0-.5.4-.9.9-.9h2.95c.24 0 .47.1.64.27L7.1 4.85c.17.17.4.27.64.27H13.6c.5 0 .9.4.9.9v6.18c0 .5-.4.9-.9.9H2.4a.9.9 0 01-.9-.9V4.4z"
              stroke="currentColor"
              stroke-width="1.3"
              stroke-linejoin="round"
            />
          </svg>
        {:else}
          <svg viewBox="0 0 16 16" width="14" height="14" fill="none">
            <path
              d="M9 1.8H4.2a.9.9 0 00-.9.9v10.6c0 .5.4.9.9.9h7.6a.9.9 0 00.9-.9V5.4L9 1.8z"
              stroke="currentColor"
              stroke-width="1.3"
              stroke-linejoin="round"
            />
            <path d="M9 1.8v3.6h3.6" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round" />
          </svg>
        {/if}
      </span>
      {#each pathInfo.segs as seg, i (i)}
        {#if i > 0}
          <span class="path-sep" aria-hidden="true">›</span>
        {/if}
        <span class="path-seg" class:last={i === pathInfo.segs.length - 1}>{seg}</span>
      {/each}
    </div>

    <span class="spacer"></span>

    <span class="meta">
      <span class="meta-num">{model.rows.length}</span>
      <span class="meta-label">{model.rows.length === 1 ? "row" : "rows"}</span>
    </span>
    <span class="meta-sep" aria-hidden="true"></span>
    <span class="meta">
      <span class="meta-num">{model.schema.columns.length}</span>
      <span class="meta-label">{model.schema.columns.length === 1 ? "field" : "fields"}</span>
    </span>

    {#if hasUnsaved}
      <span class="dirty-pill" transition:fade={{ duration: 120 }}>
        <span class="dirty-dot" aria-hidden="true"></span>
        unsaved
      </span>
    {/if}

    <button class="ghost-btn" onclick={openAddColumn} title="Add column">
      <svg viewBox="0 0 16 16" width="13" height="13" fill="none" aria-hidden="true">
        <path d="M2.5 4.5h11M2.5 8h7M2.5 11.5h11" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/>
        <path d="M11.5 7v4M9.5 9h4" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/>
      </svg>
      <span>Column</span>
    </button>
    <button class="ghost-btn" onclick={addInlineRow} title={model.mode.kind === "folder" ? "Add file" : "Add row"}>
      <svg viewBox="0 0 16 16" width="13" height="13" fill="none" aria-hidden="true">
        <path d="M2.5 4.5h11M2.5 8h11M2.5 11.5h7" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/>
        <path d="M11.5 11v4M9.5 13h4" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/>
      </svg>
      <span>{model.mode.kind === "folder" ? "File" : "Row"}</span>
    </button>

    <button
      class="save-btn"
      disabled={saving || !hasUnsaved}
      onclick={doSave}
      aria-busy={saving}
      title="Save All ({navigator.platform.toLowerCase().includes('mac') ? '⌘' : 'Ctrl'}+S)"
    >
      {#if saving}
        <span class="spin" aria-hidden="true"></span>
        <span>Saving</span>
      {:else}
        <svg viewBox="0 0 16 16" width="13" height="13" fill="none" aria-hidden="true">
          <path d="M3.5 2.5h7L13 5v8.5a1 1 0 01-1 1H4a1 1 0 01-1-1v-10a1 1 0 011-1z" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round" />
          <path d="M5.5 2.5v3.2h5V2.5M5.5 14.5v-4.4h5v4.4" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round" />
        </svg>
        <span>Save All</span>
        <kbd class="kbd-hint" aria-hidden="true">⌘S</kbd>
      {/if}
    </button>
  </header>

  <!-- Warnings -->
  {#if visibleWarnings.length > 0}
    <div class="warnings">
      {#each visibleWarnings as { w, i } (i)}
        <div
          class="warning"
          class:warn={w.kind === "unparseable_file"}
          class:info={w.kind === "empty_columns"}
          transition:fade={{ duration: 140 }}
        >
          <span class="warning-icon" aria-hidden="true">
            {#if w.kind === "empty_columns"}
              <svg viewBox="0 0 16 16" width="13" height="13" fill="none">
                <circle cx="8" cy="8" r="6.5" stroke="currentColor" stroke-width="1.4" />
                <path d="M8 5v3.5M8 11v.4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
              </svg>
            {:else}
              <svg viewBox="0 0 16 16" width="13" height="13" fill="none">
                <path d="M8 1.5l7 12.5H1L8 1.5z" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round" />
                <path d="M8 6.5v3.5M8 12v.4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
              </svg>
            {/if}
          </span>
          <span class="warning-body">
            {#if w.kind === "empty_columns"}
              <strong>All-empty columns.</strong>
              These have no values in any row:
              {#each w.names as n, ix}<code>{n}</code>{ix < w.names.length - 1 ? " " : ""}{/each}
            {:else if w.kind === "unparseable_file"}
              <strong>{basename(w.path)}</strong> &nbsp;— {w.message}
            {/if}
          </span>
          <button
            class="warning-close"
            onclick={() => {
              dismissedWarnings.add(i);
              dismissedWarnings = new Set(dismissedWarnings);
            }}
            aria-label="Dismiss"
          >
            <svg viewBox="0 0 16 16" width="11" height="11" fill="none">
              <path d="M3 3l10 10M13 3L3 13" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
            </svg>
          </button>
        </div>
      {/each}
    </div>
  {/if}

  <!-- Grid / Empty state -->
  {#if !hasColumns}
    <div class="empty-state">
      <div class="empty-card">
        <span class="empty-mark" aria-hidden="true">
          <svg viewBox="0 0 32 32" width="28" height="28" fill="none">
            <rect x="3" y="6" width="26" height="20" rx="2" stroke="currentColor" stroke-width="1.4" />
            <path d="M10 6v20M22 6v20M3 13h26M3 19h26" stroke="currentColor" stroke-width="1.2" />
          </svg>
        </span>
        <h2>No columns yet.</h2>
        <p>
          {#if model.mode.kind === "folder"}
            The <code>.md</code> files in this folder don't have any frontmatter
            fields. Add a column to start filling in values — Save All will
            write a YAML frontmatter block to each file.
          {:else}
            This file parses as a list, but the records have no fields. Add a
            column to start filling in values.
          {/if}
        </p>
        <button class="empty-cta" onclick={openAddColumn}>
          <svg viewBox="0 0 16 16" width="13" height="13" fill="none" aria-hidden="true">
            <path d="M8 3.5v9M3.5 8h9" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>
          </svg>
          <span>Add a column</span>
        </button>
      </div>
    </div>
  {:else}
    <div class="grid-wrap">
      <table>
        <thead>
          <tr>
            <th class="row-head">
              {model.mode.kind === "folder" ? "file" : "#"}
            </th>
            {#each model.schema.columns as col (col.name)}
              <th
                class="col-head type-{col.type}"
                title="{col.type} field — click to edit"
                onclick={() => openEditColumn(col)}
              >
                <span class="col-glyph type-{col.type}" aria-hidden="true">
                  {#if col.type === "text"}
                    <svg viewBox="0 0 14 14" width="12" height="12" fill="none">
                      <path d="M2.5 4V3h9v1M7 3v8.5M5 11.5h4" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>
                    </svg>
                  {:else if col.type === "number"}
                    <svg viewBox="0 0 14 14" width="12" height="12" fill="none">
                      <path d="M5 2.5L4 11.5M9 2.5L8 11.5M2.5 5h9.5M2 9h9.5" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>
                    </svg>
                  {:else if col.type === "boolean"}
                    <svg viewBox="0 0 14 14" width="12" height="12" fill="none">
                      <rect x="2.5" y="2.5" width="9" height="9" rx="1.6" stroke="currentColor" stroke-width="1.3"/>
                      <path d="M5 7.4l1.6 1.5L9 5.5" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/>
                    </svg>
                  {:else if col.type === "date"}
                    <svg viewBox="0 0 14 14" width="12" height="12" fill="none">
                      <rect x="2" y="3.4" width="10" height="8.6" rx="1.2" stroke="currentColor" stroke-width="1.3"/>
                      <path d="M2 6.2h10M4.6 2v2.4M9.4 2v2.4" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>
                    </svg>
                  {:else if col.type === "list"}
                    <svg viewBox="0 0 14 14" width="12" height="12" fill="none">
                      <circle cx="3" cy="4" r="1" fill="currentColor"/>
                      <circle cx="3" cy="7" r="1" fill="currentColor"/>
                      <circle cx="3" cy="10" r="1" fill="currentColor"/>
                      <path d="M5.5 4h6M5.5 7h6M5.5 10h4" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>
                    </svg>
                  {:else}
                    <svg viewBox="0 0 14 14" width="12" height="12" fill="none">
                      <path d="M5 3L2 7l3 4M9 3l3 4-3 4" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>
                    </svg>
                  {/if}
                </span>
                <span class="col-name">{col.name}</span>
                <button
                  type="button"
                  class="col-delete"
                  onclick={(e) => { e.stopPropagation(); requestDeleteColumn(col.name); }}
                  title="Delete column"
                  aria-label="Delete column {col.name}"
                >
                  <svg viewBox="0 0 16 16" width="11" height="11" fill="none">
                    <path d="M3 3l10 10M13 3L3 13" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>
                  </svg>
                </button>
              </th>
            {/each}
            <th class="col-add">
              <button
                type="button"
                class="col-add-btn"
                onclick={openAddColumn}
                title="Add column"
                aria-label="Add column"
              >
                <svg viewBox="0 0 16 16" width="13" height="13" fill="none">
                  <path d="M8 3.5v9M3.5 8h9" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>
                </svg>
              </button>
            </th>
          </tr>
        </thead>
        <tbody>
          {#each model.rows as row, rowIdx (row.id)}
            <tr class:errored={row.parse_error !== null} class:pending-delete={row.pending_delete}>
              <td class="row-head">
                <span class="row-num">{rowIdx + 1}</span>
                {#if model.mode.kind === "folder"}
                  <span class="row-name" title={row.source.kind === "file" ? row.source.path : ""}>
                    {rowLabel(rowIdx)}
                  </span>
                {/if}
                <button
                  type="button"
                  class="row-delete"
                  onclick={() => toggleDeleteRow(rowIdx)}
                  title={row.pending_delete ? "Restore row" : "Delete row"}
                  aria-label={row.pending_delete ? "Restore row" : "Delete row"}
                >
                  {#if row.pending_delete}
                    <svg viewBox="0 0 16 16" width="11" height="11" fill="none">
                      <path d="M3 8h10M8 3v10" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>
                    </svg>
                  {:else}
                    <svg viewBox="0 0 16 16" width="11" height="11" fill="none">
                      <path d="M3 3l10 10M13 3L3 13" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>
                    </svg>
                  {/if}
                </button>
              </td>
              {#each model.schema.columns as col (col.name)}
                {@const v = row.record.fields[col.name] ?? ({ kind: "null" } as Value)}
                {@const ckey = cellKey(rowIdx, col.name)}
                {@const dirty = dirtyCells.has(ckey)}
                {@const empty = isEmpty(v)}
                {@const invalidMsg = invalidCells.get(ckey)}
                <td
                  class="cell type-{col.type}"
                  class:dirty
                  class:empty
                  class:invalid={invalidMsg !== undefined}
                  title={invalidMsg ?? undefined}
                  data-cell="{rowIdx}::{col.name}"
                >
                  {#if row.parse_error}
                    <span class="parse-err">parse error</span>
                  {:else if col.type === "boolean"}
                    <label class="check-wrap">
                      <input
                        type="checkbox"
                        checked={v.kind === "bool" && v.value}
                        onchange={(e) =>
                          commitEdit(
                            rowIdx,
                            col.name,
                            (e.currentTarget as HTMLInputElement).checked ? "true" : "false",
                            col.type,
                          )}
                      />
                      <span class="check-box" aria-hidden="true">
                        <svg viewBox="0 0 12 12" width="10" height="10" fill="none">
                          <path d="M2.5 6.2l2.4 2.3L9.5 3.6" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" />
                        </svg>
                      </span>
                    </label>
                  {:else if col.type === "date"}
                    <DatePicker
                      value={cellText(rowIdx, col.name)}
                      onChange={(next) => commitEdit(rowIdx, col.name, next, col.type)}
                    />
                  {:else}
                    <div class="cell-edit">
                      <!-- Display layer drives column width and cell height. -->
                      {#if col.type === "list" && v.kind === "list" && v.value.length > 0}
                        <div class="display pills" aria-hidden="true">
                          {#each listItems(v) as t (t)}
                            <span class="pill" data-color={tagColor(t)}>{t}</span>
                          {/each}
                        </div>
                      {:else if empty}
                        <div class="display is-empty" aria-hidden="true">&#8211;</div>
                      {:else}
                        <div class="display" aria-hidden="true">{cellText(rowIdx, col.name)}</div>
                      {/if}
                      <!-- Interaction overlay: transparent text when not focused. -->
                      {#if col.type === "number"}
                        <input
                          type="text"
                          class="overlay"
                          value={cellText(rowIdx, col.name)}
                          onfocus={(e) =>
                            rememberOriginal(rowIdx, col.name, (e.currentTarget as HTMLInputElement).value)}
                          onkeydown={(e) => cellKeyDown(e, rowIdx, col.name, col.type)}
                          onchange={(e) =>
                            commitEdit(
                              rowIdx,
                              col.name,
                              (e.currentTarget as HTMLInputElement).value,
                              col.type,
                            )}
                          onblur={(e) =>
                            commitEdit(
                              rowIdx,
                              col.name,
                              (e.currentTarget as HTMLInputElement).value,
                              col.type,
                            )}
                        />
                      {:else}
                        <textarea
                          class="overlay"
                          rows="1"
                          value={cellText(rowIdx, col.name)}
                          onfocus={(e) =>
                            rememberOriginal(rowIdx, col.name, (e.currentTarget as HTMLTextAreaElement).value)}
                          onkeydown={(e) => cellKeyDown(e, rowIdx, col.name, col.type)}
                          onchange={(e) =>
                            commitEdit(
                              rowIdx,
                              col.name,
                              (e.currentTarget as HTMLTextAreaElement).value,
                              col.type,
                            )}
                          onblur={(e) =>
                            commitEdit(
                              rowIdx,
                              col.name,
                              (e.currentTarget as HTMLTextAreaElement).value,
                              col.type,
                            )}
                        ></textarea>
                      {/if}
                    </div>
                  {/if}
                  {#if dirty}
                    <span class="dirty-mark" aria-hidden="true"></span>
                  {/if}
                </td>
              {/each}
              <td class="col-add-cell" aria-hidden="true"></td>
            </tr>
          {/each}
          <tr class="add-row-tr">
            <td class="row-head add-row-head">
              <button
                type="button"
                class="add-row-plus"
                onclick={addInlineRow}
                title="Add {model.mode.kind === 'folder' ? 'file' : 'row'}"
                aria-label="Add row"
              >
                <svg viewBox="0 0 16 16" width="13" height="13" fill="none">
                  <path d="M8 3.5v9M3.5 8h9" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>
                </svg>
              </button>
            </td>
            {#each model.schema.columns as col (col.name)}
              <td class="add-row-spacer" aria-hidden="true"></td>
            {/each}
            <td class="add-row-spacer add-row-spacer-end" aria-hidden="true"></td>
          </tr>
        </tbody>
      </table>
    </div>
  {/if}

  <!-- Save toast -->
  {#if toast}
    <div
      class="toast"
      class:success={toast.kind === "success"}
      class:error={toast.kind === "error"}
      role="status"
      transition:fly={{ y: 12, duration: 220, easing: cubicOut }}
    >
      <span class="toast-icon" aria-hidden="true">
        {#if toast.kind === "success"}
          <svg viewBox="0 0 16 16" width="14" height="14" fill="none">
            <path d="M3.2 8.4l3 3 6.6-6.6" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
        {:else}
          <svg viewBox="0 0 16 16" width="14" height="14" fill="none">
            <path d="M8 1.5l7 12.5H1L8 1.5z" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round" />
            <path d="M8 6.5v3.5M8 12v.4" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
          </svg>
        {/if}
      </span>
      <span class="toast-text">{toast.text}</span>
      <button class="toast-close" onclick={() => (toast = null)} aria-label="Dismiss">
        <svg viewBox="0 0 16 16" width="11" height="11" fill="none">
          <path d="M3 3l10 10M13 3L3 13" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" />
        </svg>
      </button>
    </div>
  {/if}

  {#if addColOpen}
    <div class="modal-backdrop" onclick={closeAddColumn} role="presentation"></div>
    <div class="modal" role="dialog" aria-labelledby="add-col-title" aria-modal="true">
      <form onsubmit={(e) => { e.preventDefault(); confirmAddColumn(); }}>
        <h2 id="add-col-title">Add a column</h2>
        <label class="field">
          <span>Field name</span>
          <input
            type="text"
            bind:this={addColInput}
            bind:value={addColName}
            placeholder="e.g. status"
            autocomplete="off"
          />
        </label>
        <div class="field">
          <span>Type</span>
          <Select
            options={FIELD_TYPE_OPTIONS}
            value={addColType}
            onChange={(v) => (addColType = v as FieldType)}
            label="Field type"
          />
        </div>
        {#if addColError}
          <p class="modal-error">{addColError}</p>
        {/if}
        <div class="modal-actions">
          <button type="button" class="btn-secondary" onclick={closeAddColumn}>Cancel</button>
          <button type="submit" class="btn-primary">Add</button>
        </div>
      </form>
    </div>
  {/if}

  {#if editColOpen}
    <div class="modal-backdrop" onclick={cancelEditColumn} role="presentation"></div>
    <div class="modal" role="dialog" aria-labelledby="edit-col-title" aria-modal="true">
      <form onsubmit={(e) => { e.preventDefault(); confirmEditColumn(); }}>
        <h2 id="edit-col-title">Edit column</h2>
        <p class="modal-body">
          Renaming rewrites the field key in every row on Save All. Changing
          type doesn't migrate values — existing values that don't match the
          new type will be flagged.
        </p>
        <label class="field">
          <span>Field name</span>
          <input
            type="text"
            bind:this={editColInput}
            bind:value={editColName}
            autocomplete="off"
          />
        </label>
        <div class="field">
          <span>Type</span>
          <Select
            options={FIELD_TYPE_OPTIONS}
            value={editColType}
            onChange={(v) => (editColType = v as FieldType)}
            label="Field type"
          />
        </div>
        {#if editColError}
          <p class="modal-error">{editColError}</p>
        {/if}
        <div class="modal-actions">
          <button type="button" class="btn-secondary" onclick={cancelEditColumn}>Cancel</button>
          <button type="submit" class="btn-primary">Save</button>
        </div>
      </form>
    </div>
  {/if}

  {#if addRowOpen}
    <div class="modal-backdrop" onclick={() => (addRowOpen = false)} role="presentation"></div>
    <div class="modal" role="dialog" aria-labelledby="add-row-title" aria-modal="true">
      <form onsubmit={(e) => { e.preventDefault(); confirmAddFolderRow(); }}>
        <h2 id="add-row-title">New file</h2>
        <p class="modal-body">
          The file will be created on Save All. <code>.md</code> is appended
          automatically if you leave it off.
        </p>
        <label class="field">
          <span>Filename</span>
          <input
            type="text"
            bind:this={addRowInput}
            bind:value={addRowFilename}
            placeholder="e.g. notes-on-x.md"
            autocomplete="off"
          />
        </label>
        {#if addRowError}
          <p class="modal-error">{addRowError}</p>
        {/if}
        <div class="modal-actions">
          <button type="button" class="btn-secondary" onclick={() => (addRowOpen = false)}>Cancel</button>
          <button type="submit" class="btn-primary">Add</button>
        </div>
      </form>
    </div>
  {/if}

  {#if pendingDeleteCol !== null}
    <div class="modal-backdrop" onclick={cancelDeleteColumn} role="presentation"></div>
    <div class="modal" role="alertdialog" aria-labelledby="del-col-title" aria-modal="true">
      <h2 id="del-col-title">Delete column?</h2>
      <p class="modal-body">
        Removes <code>{pendingDeleteCol}</code> from every row. The field
        won't be written when you Save All.
      </p>
      <div class="modal-actions">
        <button type="button" class="btn-secondary" onclick={cancelDeleteColumn}>Cancel</button>
        <button type="button" class="btn-danger" onclick={confirmDeleteColumn}>Delete</button>
      </div>
    </div>
  {/if}
</div>

<style>
  .view {
    display: flex;
    flex-direction: column;
    height: 100vh;
    overflow: hidden;
    background: var(--mt-page-bg);
    position: relative;
  }
  /* Soft warm-paper atmosphere — disappears in dark mode automatically
     because the radial-gradient stops are alpha-weighted. */
  .view::before {
    content: "";
    position: absolute;
    inset: 0;
    pointer-events: none;
    background:
      radial-gradient(
        1100px 520px at 90% -10%,
        rgba(35, 131, 226, 0.04),
        transparent 60%
      ),
      radial-gradient(
        900px 480px at -10% 110%,
        rgba(203, 145, 47, 0.035),
        transparent 60%
      );
    z-index: 0;
  }
  .view > * {
    position: relative;
    z-index: 1;
  }

  /* ===== Topbar ===== */
  .topbar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px 8px 8px;
    border-bottom: 1px solid var(--mt-divider);
    background: var(--mt-page-bg);
    height: 44px;
    flex-shrink: 0;
  }
  .spacer {
    flex: 1;
  }

  .icon-btn {
    all: unset;
    width: 28px;
    height: 28px;
    border-radius: 4px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    color: var(--mt-fg-muted);
    transition: background 120ms ease, color 120ms ease;
  }
  .icon-btn:hover {
    background: var(--mt-hover);
    color: var(--mt-fg);
  }
  .icon-btn:focus-visible {
    outline: 2px solid var(--mt-accent);
    outline-offset: 1px;
  }

  .path {
    display: flex;
    align-items: center;
    gap: 6px;
    font-family: var(--mt-font-mono);
    font-size: 12.5px;
    color: var(--mt-fg-muted);
    overflow: hidden;
    min-width: 0;
    padding: 4px 8px;
    border-radius: 4px;
  }
  .path-icon {
    color: var(--mt-fg-subtle);
    display: inline-flex;
    flex-shrink: 0;
  }
  .path-seg {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .path-seg.last {
    color: var(--mt-fg);
    font-weight: 500;
  }
  .path-sep {
    color: var(--mt-fg-subtle);
    font-family: var(--mt-font-sans);
  }

  .meta {
    display: inline-flex;
    align-items: baseline;
    gap: 4px;
    font-size: 12px;
    color: var(--mt-fg-muted);
  }
  .meta-num {
    font-family: var(--mt-font-mono);
    font-weight: 500;
    color: var(--mt-fg);
    font-size: 12.5px;
  }
  .meta-sep {
    width: 1px;
    height: 14px;
    background: var(--mt-divider);
  }

  .dirty-pill {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    padding: 3px 10px 3px 8px;
    background: transparent;
    color: var(--mt-fg-muted);
    border: 1px solid var(--mt-border-strong);
    border-radius: 999px;
    font-family: var(--mt-font-mono);
    font-size: 10.5px;
    font-weight: 500;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .dirty-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--mt-warn);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--mt-warn) 22%, transparent);
    animation: dirty-pulse 2.4s ease-in-out infinite;
  }
  @keyframes dirty-pulse {
    0%, 100% { box-shadow: 0 0 0 3px color-mix(in srgb, var(--mt-warn) 22%, transparent); }
    50% { box-shadow: 0 0 0 5px color-mix(in srgb, var(--mt-warn) 8%, transparent); }
  }

  .save-btn {
    all: unset;
    cursor: pointer;
    padding: 5px 12px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: var(--mt-accent);
    color: #fff;
    font-size: 13px;
    font-weight: 500;
    border-radius: 4px;
    transition: background 140ms ease, opacity 140ms ease;
  }
  .save-btn:hover:not(:disabled) {
    background: var(--mt-accent-hover);
  }
  .save-btn:focus-visible {
    outline: 2px solid var(--mt-accent);
    outline-offset: 2px;
  }
  .save-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
    background: var(--mt-surface-strong);
    color: var(--mt-fg-subtle);
  }
  .save-btn .spin {
    width: 11px;
    height: 11px;
    border-radius: 50%;
    border: 1.5px solid rgba(255, 255, 255, 0.35);
    border-top-color: #fff;
    animation: spin 720ms linear infinite;
  }
  .kbd-hint {
    display: inline-flex;
    align-items: center;
    margin-left: 2px;
    padding: 1px 5px;
    border-radius: 3px;
    background: rgba(255, 255, 255, 0.18);
    font-family: var(--mt-font-mono);
    font-size: 10px;
    font-weight: 500;
    letter-spacing: 0.04em;
    color: rgba(255, 255, 255, 0.86);
  }
  .save-btn:disabled .kbd-hint {
    background: var(--mt-divider);
    color: var(--mt-fg-subtle);
  }

  /* ===== Warnings ===== */
  .warnings {
    display: flex;
    flex-direction: column;
    gap: 1px;
    background: var(--mt-divider);
    flex-shrink: 0;
  }
  .warning {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 10px 14px;
    font-size: 12.5px;
    line-height: 1.5;
    background: var(--mt-warn-bg);
    color: var(--mt-warn-fg);
  }
  .warning.info {
    background: var(--mt-warn-bg);
    color: var(--mt-warn-fg);
  }
  .warning-icon {
    margin-top: 2px;
    flex-shrink: 0;
    color: var(--mt-warn);
  }
  .warning-body {
    flex: 1;
  }
  .warning-body strong {
    font-weight: 600;
    color: inherit;
  }
  .warning-body code {
    font-family: var(--mt-font-mono);
    font-size: 11.5px;
    background: rgba(0, 0, 0, 0.04);
    border: 1px solid rgba(0, 0, 0, 0.06);
    padding: 0 5px;
    border-radius: 3px;
    margin: 0 2px;
    color: inherit;
  }
  .warning-close {
    all: unset;
    cursor: pointer;
    width: 20px;
    height: 20px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: 3px;
    color: inherit;
    opacity: 0.6;
    flex-shrink: 0;
  }
  .warning-close:hover {
    opacity: 1;
    background: rgba(0, 0, 0, 0.05);
  }

  /* ===== Empty state ===== */
  .empty-state {
    flex: 1;
    display: grid;
    place-items: center;
    padding: 32px;
    overflow: auto;
  }
  .empty-card {
    max-width: 420px;
    text-align: center;
    color: var(--mt-fg-muted);
  }
  .empty-mark {
    display: inline-flex;
    margin-bottom: 14px;
    color: var(--mt-fg-subtle);
  }
  .empty-card h2 {
    margin: 0 0 8px;
    font-family: var(--mt-font-display);
    font-weight: 600;
    font-size: 18px;
    color: var(--mt-fg);
    letter-spacing: -0.012em;
  }
  .empty-card p {
    margin: 0;
    font-size: 13.5px;
    line-height: 1.65;
  }
  .empty-card code {
    font-family: var(--mt-font-mono);
    font-size: 12px;
    background: var(--mt-surface-strong);
    padding: 0 4px;
    border-radius: 3px;
    color: var(--mt-fg);
  }
  .empty-cta {
    all: unset;
    margin-top: 18px;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    background: var(--mt-accent);
    color: #fff;
    font-size: 13px;
    font-weight: 500;
    border-radius: 4px;
    transition: background 140ms ease;
  }
  .empty-cta:hover {
    background: var(--mt-accent-hover);
  }
  .empty-cta:focus-visible {
    outline: 2px solid var(--mt-accent);
    outline-offset: 2px;
  }

  /* ===== Modal ===== */
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background:
      radial-gradient(80% 60% at 50% 40%, rgba(15, 15, 15, 0.22), rgba(15, 15, 15, 0.46));
    backdrop-filter: blur(6px) saturate(1.05);
    -webkit-backdrop-filter: blur(6px) saturate(1.05);
    z-index: 999;
    animation: fade-in 180ms ease-out;
  }
  .modal {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: min(440px, calc(100vw - 32px));
    background: var(--mt-elevated);
    color: var(--mt-fg);
    border: 1px solid var(--mt-border-strong);
    border-radius: 12px;
    padding: 0;
    z-index: 1000;
    box-shadow: var(--mt-shadow-3);
    animation: pop-in 220ms cubic-bezier(0.2, 0.9, 0.3, 1.05);
  }
  /* Hairline accent at the top — subtle wash of the brand blue.
     Rounded only at the top corners so it follows the modal frame
     without forcing overflow:hidden on the parent (which would clip
     popovers like the type dropdown). */
  .modal::before {
    content: "";
    display: block;
    height: 3px;
    border-radius: 12px 12px 0 0;
    background: linear-gradient(90deg, transparent 0%, var(--mt-accent) 30%, var(--mt-accent) 70%, transparent 100%);
    opacity: 0.55;
  }
  .modal form,
  .modal > h2,
  .modal > .modal-body,
  .modal > .modal-actions {
    padding-left: 26px;
    padding-right: 26px;
  }
  .modal form {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding-top: 22px;
    padding-bottom: 18px;
  }
  .modal h2 {
    margin: 0;
    font-family: var(--mt-font-display);
    font-weight: 600;
    font-size: 19px;
    line-height: 1.2;
    letter-spacing: -0.014em;
    color: var(--mt-fg);
  }
  .modal .modal-body {
    margin: 0;
    color: var(--mt-fg-muted);
    font-size: 13px;
    line-height: 1.55;
  }
  .modal .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 12.5px;
    color: var(--mt-fg-muted);
  }
  .modal .field > span {
    font-family: var(--mt-font-mono);
    font-size: 10.5px;
    font-weight: 500;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--mt-fg-subtle);
  }
  .modal input[type="text"] {
    font: inherit;
    font-size: 14px;
    padding: 9px 12px;
    background: var(--mt-surface);
    color: var(--mt-fg);
    border: 1px solid var(--mt-border);
    border-radius: 6px;
    outline: none;
    transition: border-color 140ms ease, background 140ms ease, box-shadow 140ms ease;
  }
  .modal input[type="text"]:focus {
    border-color: var(--mt-accent);
    background: var(--mt-elevated);
    box-shadow: 0 0 0 3px var(--mt-accent-soft);
  }
  .modal-error {
    margin: 0;
    font-size: 12.5px;
    line-height: 1.5;
    color: var(--mt-error-fg);
    background: var(--mt-error-bg);
    padding: 8px 12px;
    border-radius: 6px;
    border: 1px solid color-mix(in srgb, var(--mt-error) 22%, transparent);
  }
  .modal-actions {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 6px;
    padding-top: 14px;
    padding-bottom: 0;
    border-top: 1px solid var(--mt-divider);
    margin-left: -26px;
    margin-right: -26px;
    padding-left: 22px;
    padding-right: 22px;
  }
  .modal-actions::before {
    content: "esc to close";
    font-family: var(--mt-font-mono);
    font-size: 10.5px;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--mt-fg-subtle);
    margin-right: auto;
    padding: 2px 8px;
    border: 1px solid var(--mt-divider);
    border-radius: 4px;
  }
  .btn-primary,
  .btn-secondary,
  .btn-danger {
    all: unset;
    cursor: pointer;
    padding: 7px 16px;
    font-size: 13px;
    font-weight: 500;
    letter-spacing: -0.005em;
    border-radius: 6px;
    transition: background 140ms ease, transform 140ms ease, box-shadow 140ms ease;
  }
  .btn-primary {
    background: var(--mt-accent);
    color: #fff;
    box-shadow: inset 0 -1px 0 rgba(0, 0, 0, 0.08), 0 1px 2px rgba(0, 0, 0, 0.06);
  }
  .btn-primary:hover {
    background: var(--mt-accent-hover);
  }
  .btn-primary:active {
    transform: translateY(0.5px);
  }
  .btn-danger {
    background: var(--mt-error);
    color: #fff;
    box-shadow: inset 0 -1px 0 rgba(0, 0, 0, 0.08), 0 1px 2px rgba(0, 0, 0, 0.06);
  }
  .btn-danger:hover {
    background: color-mix(in srgb, var(--mt-error) 86%, black);
  }
  .btn-danger:active {
    transform: translateY(0.5px);
  }
  .modal-body {
    margin: 0;
    font-size: 13px;
    line-height: 1.55;
    color: var(--mt-fg-muted);
  }
  .modal-body code {
    font-family: var(--mt-font-mono);
    font-size: 12px;
    background: var(--mt-surface-strong);
    padding: 1px 5px;
    border-radius: 3px;
    color: var(--mt-fg);
    margin: 0 1px;
  }
  .btn-secondary {
    background: var(--mt-surface);
    color: var(--mt-fg);
    border: 1px solid var(--mt-border-strong);
  }
  .btn-secondary:hover {
    background: var(--mt-hover);
  }
  .btn-primary:focus-visible,
  .btn-secondary:focus-visible {
    outline: 2px solid var(--mt-accent);
    outline-offset: 2px;
  }
  @keyframes fade-in {
    from { opacity: 0; }
    to { opacity: 1; }
  }
  @keyframes pop-in {
    from {
      opacity: 0;
      transform: translate(-50%, -48%) scale(0.97);
    }
    to {
      opacity: 1;
      transform: translate(-50%, -50%) scale(1);
    }
  }

  /* ===== Table grid ===== */
  .grid-wrap {
    flex: 1;
    overflow: auto;
    background: var(--mt-page-bg);
  }
  table {
    border-collapse: separate;
    border-spacing: 0;
    width: auto;
    table-layout: auto;
    font-size: 13px;
  }
  thead th {
    position: sticky;
    top: 0;
    z-index: 3;
    background: var(--mt-surface);
    text-align: left;
    padding: 9px 12px 9px 10px;
    border-bottom: 1px solid var(--mt-border-strong);
    font-weight: 500;
    color: var(--mt-fg);
    white-space: nowrap;
    max-width: 360px;
  }
  thead::after {
    /* fine accent rule under the header, gives the band a soft underline weight */
    content: "";
  }
  thead th:not(:last-child) {
    border-right: 1px solid var(--mt-divider);
  }
  thead th.col-head {
    display: table-cell;
    vertical-align: middle;
  }
  thead th.col-head .col-glyph {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    margin-right: 7px;
    border-radius: 5px;
    background: var(--mt-elevated);
    border: 1px solid var(--mt-divider);
    vertical-align: -5px;
    color: var(--mt-fg-muted);
    transition: color 140ms ease, background 140ms ease, border-color 140ms ease;
  }
  /* Per-type accent — a quiet hue, only on the glyph badge */
  thead th.col-head.type-text .col-glyph    { color: var(--mt-tag-blue-fg); background: var(--mt-tag-blue); border-color: transparent; }
  thead th.col-head.type-number .col-glyph  { color: var(--mt-tag-green-fg); background: var(--mt-tag-green); border-color: transparent; }
  thead th.col-head.type-boolean .col-glyph { color: var(--mt-tag-purple-fg); background: var(--mt-tag-purple); border-color: transparent; }
  thead th.col-head.type-date .col-glyph    { color: var(--mt-tag-orange-fg); background: var(--mt-tag-orange); border-color: transparent; }
  thead th.col-head.type-list .col-glyph    { color: var(--mt-tag-pink-fg); background: var(--mt-tag-pink); border-color: transparent; }
  thead th.col-head.type-raw .col-glyph     { color: var(--mt-tag-gray-fg); background: var(--mt-tag-gray); border-color: transparent; }
  thead th .col-name {
    display: inline-block;
    font-family: var(--mt-font-display);
    font-size: 13px;
    font-weight: 600;
    letter-spacing: -0.005em;
    color: var(--mt-fg);
    vertical-align: middle;
  }
  thead th.col-head:hover {
    background: var(--mt-surface-strong);
    cursor: pointer;
  }
  /* Per-column delete (×) — only visible on header hover. */
  thead th.col-head {
    position: sticky;
    top: 0;
    padding-right: 28px;
  }
  .col-delete {
    all: unset;
    position: absolute;
    top: 6px;
    right: 6px;
    width: 18px;
    height: 18px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: 3px;
    color: var(--mt-fg-subtle);
    cursor: pointer;
    opacity: 0;
    transition: opacity 120ms ease, background 120ms ease, color 120ms ease;
  }
  thead th.col-head:hover .col-delete,
  .col-delete:focus-visible {
    opacity: 1;
  }
  .col-delete:hover {
    background: var(--mt-error-bg);
    color: var(--mt-error);
  }
  .col-delete:focus-visible {
    outline: 2px solid var(--mt-accent);
    outline-offset: 1px;
  }
  /* Trailing "+" header */
  thead th.col-add {
    padding: 4px;
    background: var(--mt-surface);
    border-right: none;
    border-bottom: 1px solid var(--mt-border-strong);
    width: 38px;
  }
  .col-add-btn {
    all: unset;
    cursor: pointer;
    width: 26px;
    height: 26px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: 4px;
    color: var(--mt-fg-subtle);
    transition: background 120ms ease, color 120ms ease;
  }
  .col-add-btn:hover {
    background: var(--mt-hover);
    color: var(--mt-fg);
  }
  .col-add-btn:focus-visible {
    outline: 2px solid var(--mt-accent);
    outline-offset: 1px;
  }
  td.col-add-cell {
    background: var(--mt-page-bg);
    border-bottom: 1px solid var(--mt-divider);
    padding: 0;
    min-height: 36px;
  }

  /* Bottom "Add row" plus, symmetric to the trailing column "+" header */
  tr.add-row-tr td {
    border: none;
    background: var(--mt-page-bg);
    padding: 0;
  }
  td.add-row-head {
    background: var(--mt-surface) !important;
    padding: 4px 6px !important;
  }
  .add-row-plus {
    all: unset;
    cursor: pointer;
    width: 26px;
    height: 26px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: 4px;
    color: var(--mt-fg-subtle);
    transition: background 120ms ease, color 120ms ease;
  }
  .add-row-plus:hover {
    background: var(--mt-hover);
    color: var(--mt-fg);
  }
  .add-row-plus:focus-visible {
    outline: 2px solid var(--mt-accent);
    outline-offset: 1px;
  }
  td.add-row-spacer {
    height: 36px;
    border-right: 1px solid transparent;
  }

  /* Invalid cell — soft red ring inside the cell border */
  .cell.invalid {
    box-shadow: inset 0 0 0 1px var(--mt-error);
    background: color-mix(in srgb, var(--mt-error-bg) 70%, transparent) !important;
  }
  .cell.invalid::after {
    content: "";
    position: absolute;
    top: 6px;
    right: 6px;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--mt-error);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--mt-error) 24%, transparent);
    pointer-events: none;
  }

  /* Per-row delete (shown on row hover, top-right of row-head) */
  .row-delete {
    all: unset;
    position: absolute;
    top: 6px;
    right: 6px;
    width: 18px;
    height: 18px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: 3px;
    color: var(--mt-fg-subtle);
    cursor: pointer;
    opacity: 0;
    transition: opacity 120ms ease, background 120ms ease, color 120ms ease;
  }
  tr:hover .row-delete,
  .row-delete:focus-visible,
  tr.pending-delete .row-delete {
    opacity: 1;
  }
  .row-delete:hover {
    background: var(--mt-error-bg);
    color: var(--mt-error);
  }
  tr.pending-delete .row-delete {
    color: var(--mt-error);
  }
  /* Strikethrough + dim a row marked for deletion. Cells stay editable so the
     user can change their mind without losing data. */
  tr.pending-delete td {
    background: var(--mt-error-bg);
    opacity: 0.55;
    text-decoration: line-through;
    text-decoration-color: var(--mt-error);
  }
  tr.pending-delete td.row-head {
    background: var(--mt-error-bg);
  }

  /* Toolbar "Add row" button (ghost variant) */
  .ghost-btn {
    all: unset;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 5px 10px;
    color: var(--mt-fg-muted);
    font-size: 12.5px;
    font-weight: 500;
    border-radius: 4px;
    border: 1px solid var(--mt-border);
    background: var(--mt-surface);
    transition: background 120ms ease, color 120ms ease, border-color 120ms ease;
  }
  .ghost-btn:hover {
    background: var(--mt-surface-strong);
    color: var(--mt-fg);
    border-color: var(--mt-border-strong);
  }
  .ghost-btn:focus-visible {
    outline: 2px solid var(--mt-accent);
    outline-offset: 2px;
  }

  /* Sticky row-head column (filename / index) */
  th.row-head,
  td.row-head {
    position: sticky;
    left: 0;
    z-index: 2;
    background: var(--mt-surface);
    padding: 9px 28px 9px 12px;
    max-width: 280px;
    font-family: var(--mt-font-mono);
    font-size: 11.5px;
    color: var(--mt-fg-muted);
    border-right: 1px solid var(--mt-border);
    white-space: nowrap;
    vertical-align: top;
  }
  thead th.row-head {
    z-index: 4;
    background: var(--mt-surface);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--mt-fg-subtle);
    font-family: var(--mt-font-mono);
    font-size: 10px;
    border-bottom: 1px solid var(--mt-border-strong);
  }
  td.row-head {
    min-height: 36px;
  }
  .row-num {
    display: inline-block;
    color: var(--mt-fg-subtle);
    font-size: 10.5px;
    text-align: right;
    min-width: 18px;
    vertical-align: top;
  }
  .row-name {
    display: inline-block;
    margin-left: 8px;
    color: var(--mt-fg);
    font-size: 12px;
    vertical-align: top;
    max-width: 220px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  tbody td {
    border-bottom: 1px solid var(--mt-divider);
    border-right: 1px solid var(--mt-divider);
    padding: 0;
    min-height: 36px;
    height: auto;
    max-width: 360px;
    vertical-align: top;
    position: relative;
    background: var(--mt-page-bg);
    overflow-wrap: break-word;
  }
  tbody tr:hover td {
    background: var(--mt-hover);
  }
  tbody tr:hover td.row-head {
    background: var(--mt-surface-strong);
  }
  tbody tr.errored td {
    background: var(--mt-error-bg);
  }
  tbody td:last-child {
    border-right: 0;
  }

  /* Cell editor — display layer + transparent overlay.
     The display <div> drives column width + cell height (it sizes to content).
     The <input>/<textarea> overlay is positioned over it; transparent text
     when not focused, normal when focused. */
  .cell-edit {
    position: relative;
    width: 100%;
    min-height: 36px;
  }
  .cell-edit .display {
    padding: 8px 12px;
    min-height: 36px;
    line-height: 1.5;
    white-space: pre-wrap;
    overflow-wrap: break-word;
    color: var(--mt-fg);
    font-family: inherit;
  }
  .cell.type-number .cell-edit .display {
    text-align: right;
    font-family: var(--mt-font-mono);
    font-size: 12.5px;
    font-feature-settings: "tnum" on, "lnum" on;
  }
  .cell.type-date .cell-edit .display {
    font-family: var(--mt-font-mono);
    font-size: 12.5px;
    letter-spacing: 0.01em;
    white-space: nowrap;
  }
  .cell.type-raw .cell-edit .display {
    font-family: var(--mt-font-mono);
    font-size: 12px;
    color: var(--mt-fg-muted);
  }
  .cell-edit .display.is-empty {
    color: var(--mt-fg-subtle);
    user-select: none;
  }
  .cell-edit .display.pills {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    align-content: center;
    align-items: center;
    padding: 7px 12px;
  }

  .cell-edit .overlay {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    border: 0;
    outline: 0;
    appearance: none;
    -webkit-appearance: none;
    background: transparent;
    padding: 8px 12px;
    margin: 0;
    font: inherit;
    color: transparent;
    -webkit-text-fill-color: transparent;
    caret-color: transparent;
    box-sizing: border-box;
    resize: none;
    overflow: hidden;
    line-height: 1.5;
    text-shadow: none;
  }
  .cell-edit textarea.overlay {
    white-space: pre-wrap;
    overflow-wrap: break-word;
    font-family: inherit;
  }
  .cell.type-number .cell-edit .overlay {
    text-align: right;
    font-family: var(--mt-font-mono);
    font-size: 12.5px;
  }
  /* Date cells host the custom <DatePicker> directly — no overlay/display split. */
  .cell.type-date {
    padding: 0;
  }
  .cell.type-raw .cell-edit .overlay {
    font-family: var(--mt-font-mono);
    font-size: 12px;
  }
  .cell-edit:focus-within .display {
    visibility: hidden;
  }
  .cell-edit:focus-within .overlay {
    color: var(--mt-fg);
    -webkit-text-fill-color: var(--mt-fg);
    caret-color: var(--mt-accent);
    background: var(--mt-page-bg);
    box-shadow: inset 0 0 0 2px var(--mt-accent);
    border-radius: 1px;
    z-index: 2;
  }
  .cell-edit:focus-within .overlay::placeholder {
    color: var(--mt-fg-subtle);
    font-style: italic;
  }

  .cell.type-number input[type="text"] {
    text-align: right;
    font-family: var(--mt-font-mono);
    font-feature-settings: "tnum" on, "lnum" on;
    font-size: 12.5px;
  }
  .cell.type-date input[type="text"] {
    font-family: var(--mt-font-mono);
    font-size: 12.5px;
    color: var(--mt-fg);
    letter-spacing: 0.01em;
  }
  .cell.type-raw input[type="text"] {
    font-family: var(--mt-font-mono);
    font-size: 12px;
    color: var(--mt-fg-muted);
  }
  .cell.empty input[type="text"] {
    color: var(--mt-fg-subtle);
  }

  /* Boolean checkbox — Notion-ish square */
  .check-wrap {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
    cursor: pointer;
    position: relative;
  }
  .check-wrap input {
    position: absolute;
    inset: 0;
    opacity: 0;
    cursor: pointer;
  }
  .check-box {
    width: 16px;
    height: 16px;
    border-radius: 3px;
    border: 1.4px solid var(--mt-border-strong);
    background: var(--mt-elevated);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: transparent;
    transition: background 120ms ease, border-color 120ms ease;
  }
  .check-wrap input:checked + .check-box {
    background: var(--mt-accent);
    border-color: var(--mt-accent);
    color: #fff;
  }
  .check-wrap input:focus-visible + .check-box {
    box-shadow: 0 0 0 2px var(--mt-accent-soft);
  }
  .check-wrap:hover .check-box {
    border-color: var(--mt-fg-muted);
  }

  .pill {
    display: inline-block;
    padding: 1px 7px;
    border-radius: 3px;
    font-size: 11.5px;
    line-height: 17px;
    white-space: nowrap;
    flex-shrink: 0;
  }
  .pill[data-color="blue"] {
    background: var(--mt-tag-blue);
    color: var(--mt-tag-blue-fg);
  }
  .pill[data-color="green"] {
    background: var(--mt-tag-green);
    color: var(--mt-tag-green-fg);
  }
  .pill[data-color="yellow"] {
    background: var(--mt-tag-yellow);
    color: var(--mt-tag-yellow-fg);
  }
  .pill[data-color="orange"] {
    background: var(--mt-tag-orange);
    color: var(--mt-tag-orange-fg);
  }
  .pill[data-color="red"] {
    background: var(--mt-tag-red);
    color: var(--mt-tag-red-fg);
  }
  .pill[data-color="purple"] {
    background: var(--mt-tag-purple);
    color: var(--mt-tag-purple-fg);
  }
  .pill[data-color="pink"] {
    background: var(--mt-tag-pink);
    color: var(--mt-tag-pink-fg);
  }
  .pill[data-color="gray"] {
    background: var(--mt-tag-gray);
    color: var(--mt-tag-gray-fg);
  }

  /* Dirty marker — small dot, top-right */
  .dirty-mark {
    position: absolute;
    top: 5px;
    right: 5px;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--mt-warn);
    pointer-events: none;
    z-index: 2;
  }

  .parse-err {
    color: var(--mt-error-fg);
    font-size: 11.5px;
    padding: 0 12px;
    font-style: italic;
  }

  /* ===== Toast ===== */
  .toast {
    position: fixed;
    bottom: 22px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 200;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px 10px 14px;
    border-radius: 6px;
    box-shadow: var(--mt-shadow-2);
    max-width: min(720px, calc(100vw - 48px));
    font-size: 13px;
    font-weight: 500;
    line-height: 1.45;
  }
  .toast.success {
    background: var(--mt-success);
    color: #fff;
  }
  .toast.error {
    background: var(--mt-error);
    color: #fff;
  }
  .toast-icon {
    display: inline-flex;
    flex-shrink: 0;
    color: rgba(255, 255, 255, 0.95);
  }
  .toast-text {
    flex: 1;
    word-break: break-word;
  }
  .toast-close {
    all: unset;
    cursor: pointer;
    width: 22px;
    height: 22px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: 3px;
    color: rgba(255, 255, 255, 0.85);
    flex-shrink: 0;
  }
  .toast-close:hover {
    background: rgba(255, 255, 255, 0.18);
    color: #fff;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  /* Subtle scrollbar tuning */
  .grid-wrap::-webkit-scrollbar {
    width: 12px;
    height: 12px;
  }
  .grid-wrap::-webkit-scrollbar-track {
    background: transparent;
  }
  .grid-wrap::-webkit-scrollbar-thumb {
    background: var(--mt-border-strong);
    border: 3px solid var(--mt-page-bg);
    border-radius: 999px;
  }
  .grid-wrap::-webkit-scrollbar-thumb:hover {
    background: var(--mt-fg-subtle);
  }
</style>
