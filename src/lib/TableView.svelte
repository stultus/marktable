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
    type Row,
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
  // Per-file failure list from the most recent Save All. When non-empty,
  // a "View details" link appears in the error toast and opens a modal
  // listing each {path, message}.
  let saveFailures = $state<{ path: string; message: string }[]>([]);
  let saveFailuresOpen = $state(false);
  // Confirm-discard prompt — appears when the user clicks Close while there
  // are unsaved changes. Direct close (no unsaved) skips the prompt.
  let confirmCloseOpen = $state(false);

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

  // Reserved cellKey suffix for the filename column in folder mode.
  const FILENAME_COL = "__filename__";

  function effectiveFilename(row: Row): string {
    if (row.pending_rename != null && row.pending_rename !== "") {
      return row.pending_rename;
    }
    if (row.source.kind === "file") return basename(row.source.path);
    return "";
  }

  function validateFilename(rowIdx: number, raw: string): string | null {
    const text = raw.trim();
    if (text === "") return "Filename is required";
    if (text.includes("/") || text.includes("\\"))
      return "Filename cannot contain path separators";
    if (text.split(/[/\\]/).some((seg) => seg === ".."))
      return "Filename cannot contain '..'";
    // The save phase auto-keeps whatever extension the user types, but if they
    // strip the .md it becomes a non-markdown file in a markdown folder.
    if (!/\.md$/i.test(text)) return "Filename must end in .md";
    // Collision: any OTHER row's effective filename equals this one.
    for (let i = 0; i < model.rows.length; i++) {
      if (i === rowIdx) continue;
      const other = model.rows[i];
      if (other.pending_delete) continue;
      if (effectiveFilename(other).toLowerCase() === text.toLowerCase()) {
        return `A file named "${text}" already exists`;
      }
    }
    return null;
  }

  function commitFilenameEdit(rowIdx: number, raw: string) {
    const row = model.rows[rowIdx];
    if (row.source.kind !== "file") return;
    const text = raw.trim();
    const key = cellKey(rowIdx, FILENAME_COL);
    const err = validateFilename(rowIdx, text);
    if (err) {
      invalidCells.set(key, err);
      invalidCells = new Map(invalidCells);
      // Don't propagate an invalid filename to pending_rename — the user's
      // input stays in the input field via its own value binding.
      return;
    } else {
      invalidCells.delete(key);
      invalidCells = new Map(invalidCells);
    }
    const current = basename(row.source.path);
    if (text === current) {
      // No-op rename: clear any prior pending_rename, leave dirty alone.
      row.pending_rename = null;
      return;
    }
    row.pending_rename = text;
    rowsDirty = true;
  }

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

  function filenameKeyDown(e: KeyboardEvent, rowIdx: number) {
    const target = e.currentTarget as HTMLInputElement;
    if (e.key === "Escape") {
      e.preventDefault();
      const orig = cellOriginal.get(cellKey(rowIdx, FILENAME_COL));
      if (orig !== undefined) target.value = orig;
      target.blur();
      return;
    }
    if (e.key === "Enter") {
      e.preventDefault();
      commitFilenameEdit(rowIdx, target.value);
      // Advance to the first data cell of this row.
      const firstCol = model.schema.columns[0]?.name;
      if (firstCol) focusCell(rowIdx, firstCol);
      else target.blur();
    }
  }

  const cellKey = (row: number, col: string) => `${row}::${col}`;

  let addColInput = $state<HTMLInputElement | null>(null);
  let addRowInput = $state<HTMLInputElement | null>(null);

  // Focus management: when a modal opens we remember the trigger element,
  // and on close we return focus to it. Saves keyboard users from being
  // dumped at the document body.
  let lastFocused: HTMLElement | null = null;
  function captureTrigger() {
    const el = document.activeElement;
    lastFocused = el instanceof HTMLElement ? el : null;
  }
  function restoreFocus() {
    const el = lastFocused;
    lastFocused = null;
    // Defer one tick so the modal's tear-down doesn't reclaim focus.
    if (el && document.body.contains(el)) {
      tick().then(() => el.focus());
    }
  }

  /** Svelte action: trap Tab/Shift+Tab inside the node so focus can't escape
   *  to background controls. Wraps every modal. Cycles through tabbable
   *  descendants only; everything else is reachable via mouse or programmatic
   *  focus, but not Tab key. */
  function trapFocus(node: HTMLElement) {
    const FOCUSABLE =
      'a[href], button:not([disabled]), input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';
    function onKey(e: KeyboardEvent) {
      if (e.key !== "Tab") return;
      const items = Array.from(node.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
        (el) => el.offsetParent !== null,
      );
      if (items.length === 0) return;
      const first = items[0];
      const last = items[items.length - 1];
      const active = document.activeElement as HTMLElement | null;
      if (e.shiftKey) {
        if (active === first || !node.contains(active)) {
          e.preventDefault();
          last.focus();
        }
      } else {
        if (active === last) {
          e.preventDefault();
          first.focus();
        }
      }
    }
    node.addEventListener("keydown", onKey);
    return {
      destroy() {
        node.removeEventListener("keydown", onKey);
      },
    };
  }

  function openAddColumn() {
    captureTrigger();
    addColName = "";
    addColType = "text";
    addColError = null;
    addColOpen = true;
    tick().then(() => addColInput?.focus());
  }
  function closeAddColumn() {
    addColOpen = false;
    restoreFocus();
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
    restoreFocus();
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
      captureTrigger();
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
    restoreFocus();
  }

  function toggleDeleteRow(rowIdx: number) {
    const r = model.rows[rowIdx];
    r.pending_delete = !r.pending_delete;
    rowsDirty = true;
  }

  function openEditColumn(col: { name: string; type: FieldType }) {
    captureTrigger();
    editColOriginalName = col.name;
    editColName = col.name;
    editColType = col.type;
    editColError = null;
    editColOpen = true;
    tick().then(() => editColInput?.focus());
  }
  function cancelEditColumn() {
    editColOpen = false;
    restoreFocus();
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
    restoreFocus();
  }

  let deleteConfirmButton = $state<HTMLButtonElement | null>(null);
  function requestDeleteColumn(name: string) {
    captureTrigger();
    pendingDeleteCol = name;
    // Focus the destructive primary so Enter confirms (matches OS conventions
    // for confirm dialogs). Esc still dismisses via the global handler.
    tick().then(() => deleteConfirmButton?.focus());
  }
  function cancelDeleteColumn() {
    pendingDeleteCol = null;
    restoreFocus();
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
    restoreFocus();
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

  /** Replace a list cell's value with the given items. Empty list collapses
   *  to Value::Null so YAML/JSON write a bare null instead of an empty array. */
  function setListField(rowIdx: number, colName: string, items: string[]) {
    const current =
      model.rows[rowIdx].record.fields[colName] ?? ({ kind: "null" } as Value);
    const next: Value =
      items.length === 0
        ? { kind: "null" }
        : {
            kind: "list",
            value: items.map((s) => ({ kind: "string", value: s }) as Value),
          };
    if (JSON.stringify(next) === JSON.stringify(current)) return;
    model.rows[rowIdx].record.fields[colName] = next;
    model.rows[rowIdx].dirty = true;
    const key = cellKey(rowIdx, colName);
    dirtyCells.add(key);
    dirtyCells = new Set(dirtyCells);
  }
  function listAddItem(rowIdx: number, colName: string, raw: string) {
    const text = raw.trim();
    if (text === "") return;
    const v = model.rows[rowIdx].record.fields[colName];
    const current =
      v?.kind === "list" ? v.value.map((x) => valueToDisplay(x)) : [];
    setListField(rowIdx, colName, [...current, text]);
  }
  function removeListItem(rowIdx: number, colName: string, idx: number) {
    const v = model.rows[rowIdx].record.fields[colName];
    if (v?.kind !== "list") return;
    const current = v.value.map((x) => valueToDisplay(x));
    current.splice(idx, 1);
    setListField(rowIdx, colName, current);
  }
  function listKeyDown(
    e: KeyboardEvent,
    rowIdx: number,
    colName: string,
    items: string[],
  ) {
    const target = e.currentTarget as HTMLInputElement;
    if (e.key === "Enter" || e.key === ",") {
      e.preventDefault();
      listAddItem(rowIdx, colName, target.value);
      target.value = "";
    } else if (
      e.key === "Backspace" &&
      target.value === "" &&
      items.length > 0
    ) {
      e.preventDefault();
      removeListItem(rowIdx, colName, items.length - 1);
    } else if (e.key === "Escape") {
      e.preventDefault();
      target.value = "";
      target.blur();
    }
  }
  function listBlur(e: FocusEvent, rowIdx: number, colName: string) {
    const target = e.currentTarget as HTMLInputElement;
    if (target.value.trim() !== "") {
      listAddItem(rowIdx, colName, target.value);
      target.value = "";
    }
  }
  function focusListInput(e: MouseEvent) {
    // Click anywhere in the cell shell focuses the trailing input. Chip
    // × buttons handle their own click via stopPropagation.
    const target = e.target as HTMLElement;
    if (target.closest(".list-chip-x")) return;
    if (target.closest(".list-add")) return;
    const input = (e.currentTarget as HTMLElement).querySelector<HTMLInputElement>(
      ".list-add",
    );
    input?.focus();
  }

  /** Cycle a boolean cell: null → true → false → null. Bypasses commitEdit's
   *  text→Value parsing because we already know the next Value directly. */
  function cycleBool(row: number, col: string, current: Value) {
    let next: Value;
    if (current.kind !== "bool") {
      next = { kind: "bool", value: true };
    } else if (current.value) {
      next = { kind: "bool", value: false };
    } else {
      next = { kind: "null" };
    }
    if (JSON.stringify(next) === JSON.stringify(current)) return;
    const key = cellKey(row, col);
    model.rows[row].record.fields[col] = next;
    model.rows[row].dirty = true;
    dirtyCells.add(key);
    dirtyCells = new Set(dirtyCells);
    // Boolean values are always valid for a boolean column.
    if (invalidCells.has(key)) {
      invalidCells.delete(key);
      invalidCells = new Map(invalidCells);
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
        // Apply successful renames: update Row.source.path and clear
        // pending_rename. The renamed list comes from save_all so it's
        // authoritative — only mutate rows whose old path matches.
        if (result.renamed.length > 0) {
          for (const ren of result.renamed) {
            for (const r of model.rows) {
              if (r.source.kind === "file" && r.source.path === ren.from) {
                r.source = { kind: "file", path: ren.to, original_text: r.source.original_text };
                r.pending_rename = null;
              }
            }
          }
        }
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
        // Stash the per-file detail and surface a "View details" link in
        // the toast. Comma-separating long messages in the toast itself
        // truncates badly when there are 3+ failures.
        saveFailures = result.failures;
        toast = {
          kind: "error",
          text:
            failed === 1
              ? `Saved ${wrote}. 1 file failed: ${basename(result.failures[0].path)} — ${result.failures[0].message}`
              : `Saved ${wrote}. ${failed} files failed.`,
        };
      }
    } catch (e) {
      toast = { kind: "error", text: `Save failed: ${String(e)}` };
    } finally {
      saving = false;
    }
  }

  function openSaveFailures() {
    captureTrigger();
    saveFailuresOpen = true;
  }
  function closeSaveFailures() {
    saveFailuresOpen = false;
    restoreFocus();
  }

  /** Wraps the parent's onClose: if there are unsaved changes, show a
   *  confirm-discard prompt. If clean, close immediately. */
  function requestClose() {
    if (hasUnsaved) {
      captureTrigger();
      confirmCloseOpen = true;
    } else {
      onClose();
    }
  }
  function cancelClose() {
    confirmCloseOpen = false;
    restoreFocus();
  }
  function confirmDiscardAndClose() {
    confirmCloseOpen = false;
    onClose();
  }

  onMount(() => {
    const onKey = (e: KeyboardEvent) => {
      // Cmd/Ctrl-S → Save All
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "s") {
        e.preventDefault();
        if (!saving && hasUnsaved) doSave();
        return;
      }
      // Escape closes any open modal and returns focus to its trigger.
      if (e.key === "Escape") {
        if (addColOpen) {
          addColOpen = false;
          restoreFocus();
        } else if (editColOpen) {
          editColOpen = false;
          restoreFocus();
        } else if (addRowOpen) {
          addRowOpen = false;
          restoreFocus();
        } else if (pendingDeleteCol !== null) {
          pendingDeleteCol = null;
          restoreFocus();
        } else if (saveFailuresOpen) {
          saveFailuresOpen = false;
          restoreFocus();
        } else if (confirmCloseOpen) {
          confirmCloseOpen = false;
          restoreFocus();
        }
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
  let pendingRenameCount = $derived(
    model.rows.filter((r) => r.pending_rename != null && r.pending_rename !== "").length,
  );
  // A "new row" in folder mode is a Row whose original_text is empty — it
  // doesn't exist on disk yet. In single-file mode (Inline source), there's
  // no parallel concept; we surface row mutations via rowsDirty.
  let newRowCount = $derived(
    model.rows.filter(
      (r) => r.source.kind === "file" && r.source.original_text === "" && !r.pending_delete,
    ).length,
  );
  let hasUnsaved = $derived(dirtyCount > 0 || schemaDirty || rowsDirty);
  /** Total count for the dirty pill numeric. Sums per-cell edits, schema
   *  mutations (counted as 1), pending row deletions, pending renames, and
   *  new rows. */
  let dirtyTotal = $derived(
    dirtyCount + (schemaDirty ? 1 : 0) + pendingDeleteCount + pendingRenameCount + newRowCount,
  );
  /** Tooltip-friendly breakdown of what's unsaved. */
  let dirtyBreakdown = $derived.by(() => {
    const parts: string[] = [];
    if (dirtyCount > 0) parts.push(`${dirtyCount} cell${dirtyCount === 1 ? "" : "s"} edited`);
    if (schemaDirty) parts.push("schema changed");
    if (newRowCount > 0) parts.push(`${newRowCount} row${newRowCount === 1 ? "" : "s"} added`);
    if (pendingRenameCount > 0)
      parts.push(`${pendingRenameCount} rename${pendingRenameCount === 1 ? "" : "s"} pending`);
    if (pendingDeleteCount > 0)
      parts.push(
        `${pendingDeleteCount} row${pendingDeleteCount === 1 ? "" : "s"} marked for delete`,
      );
    return parts.length === 0 ? "no changes" : parts.join(" · ");
  });
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

  /** Per-type empty-cell placeholder. The previous universal en-dash read
   *  ambiguous in number/date columns ("is that zero or empty?"). Each type
   *  now gets a typed cue rendered in fg-subtle italic via .display.is-empty. */
  function emptyPlaceholderFor(type: FieldType): string {
    switch (type) {
      case "number":
        return "0";
      case "date":
        return "yyyy-mm-dd";
      case "list":
        return "[ ]";
      case "raw":
        return "{ }";
      case "text":
      case "boolean":
      default:
        return "–"; // en-dash
    }
  }
</script>

<div class="view">
  <!-- Topbar -->
  <header class="topbar">
    <button class="icon-btn" onclick={requestClose} aria-label="Close">
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
              stroke-width="1.4"
              stroke-linejoin="round"
            />
          </svg>
        {:else}
          <svg viewBox="0 0 16 16" width="14" height="14" fill="none">
            <path
              d="M9 1.8H4.2a.9.9 0 00-.9.9v10.6c0 .5.4.9.9.9h7.6a.9.9 0 00.9-.9V5.4L9 1.8z"
              stroke="currentColor"
              stroke-width="1.4"
              stroke-linejoin="round"
            />
            <path d="M9 1.8v3.6h3.6" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round" />
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
      <span
        class="dirty-pill"
        title={dirtyBreakdown}
        aria-label={`Unsaved changes: ${dirtyBreakdown}`}
        transition:fade={{ duration: 120 }}
      >
        <span class="dirty-dot" aria-hidden="true"></span>
        <span>{dirtyTotal}</span>
        <span class="dirty-pill-label">unsaved</span>
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
                <path d="M8 5v3.5M8 11v.4" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
              </svg>
            {:else}
              <svg viewBox="0 0 16 16" width="13" height="13" fill="none">
                <path d="M8 1.5l7 12.5H1L8 1.5z" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round" />
                <path d="M8 6.5v3.5M8 12v.4" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
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
            <path d="M10 6v20M22 6v20M3 13h26M3 19h26" stroke="currentColor" stroke-width="1.4" />
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
                      <path d="M2.5 4V3h9v1M7 3v8.5M5 11.5h4" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/>
                    </svg>
                  {:else if col.type === "number"}
                    <svg viewBox="0 0 14 14" width="12" height="12" fill="none">
                      <path d="M5 2.5L4 11.5M9 2.5L8 11.5M2.5 5h9.5M2 9h9.5" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/>
                    </svg>
                  {:else if col.type === "boolean"}
                    <svg viewBox="0 0 14 14" width="12" height="12" fill="none">
                      <rect x="2.5" y="2.5" width="9" height="9" rx="1.6" stroke="currentColor" stroke-width="1.4"/>
                      <path d="M5 7.4l1.6 1.5L9 5.5" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/>
                    </svg>
                  {:else if col.type === "date"}
                    <svg viewBox="0 0 14 14" width="12" height="12" fill="none">
                      <rect x="2" y="3.4" width="10" height="8.6" rx="1.2" stroke="currentColor" stroke-width="1.4"/>
                      <path d="M2 6.2h10M4.6 2v2.4M9.4 2v2.4" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/>
                    </svg>
                  {:else if col.type === "list"}
                    <svg viewBox="0 0 14 14" width="12" height="12" fill="none">
                      <circle cx="3" cy="4" r="1" fill="currentColor"/>
                      <circle cx="3" cy="7" r="1" fill="currentColor"/>
                      <circle cx="3" cy="10" r="1" fill="currentColor"/>
                      <path d="M5.5 4h6M5.5 7h6M5.5 10h4" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/>
                    </svg>
                  {:else}
                    <svg viewBox="0 0 14 14" width="12" height="12" fill="none">
                      <path d="M5 3L2 7l3 4M9 3l3 4-3 4" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/>
                    </svg>
                  {/if}
                </span>
                <span class="col-name">{col.name}</span>
                <span class="col-edit-hint" aria-hidden="true">
                  <svg viewBox="0 0 16 16" width="11" height="11" fill="none">
                    <path d="M2.5 13.5h3l7.6-7.6a1.4 1.4 0 000-2L12.1 2.9a1.4 1.4 0 00-2 0L2.5 10.5v3z" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"/>
                    <path d="M9 4l3 3" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/>
                  </svg>
                </span>
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
            {@const filenameKey = cellKey(rowIdx, FILENAME_COL)}
            {@const filenameInvalid = invalidCells.get(filenameKey)}
            {@const isRenamed = row.pending_rename != null && row.pending_rename !== ""}
            <tr class:errored={row.parse_error !== null} class:pending-delete={row.pending_delete}>
              <td
                class="row-head"
                class:invalid={filenameInvalid !== undefined}
                class:renamed={isRenamed}
                title={filenameInvalid ?? (row.source.kind === "file" ? row.source.path : "")}
                data-cell="{rowIdx}::{FILENAME_COL}"
              >
                <span class="row-num">{rowIdx + 1}</span>
                {#if model.mode.kind === "folder"}
                  <input
                    type="text"
                    class="row-name-input"
                    value={effectiveFilename(row)}
                    spellcheck="false"
                    autocomplete="off"
                    onfocus={(e) =>
                      rememberOriginal(rowIdx, FILENAME_COL, (e.currentTarget as HTMLInputElement).value)}
                    onkeydown={(e) => filenameKeyDown(e, rowIdx)}
                    onchange={(e) =>
                      commitFilenameEdit(rowIdx, (e.currentTarget as HTMLInputElement).value)}
                    onblur={(e) =>
                      commitFilenameEdit(rowIdx, (e.currentTarget as HTMLInputElement).value)}
                  />
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
                    <button
                      type="button"
                      class="bool-cell"
                      class:bool-true={v.kind === "bool" && v.value}
                      class:bool-false={v.kind === "bool" && !v.value}
                      class:bool-null={v.kind === "null"}
                      onclick={() => cycleBool(rowIdx, col.name, v)}
                      title="Click to cycle — empty → true → false"
                      aria-label="{col.name}: {v.kind === 'bool' ? (v.value ? 'true' : 'false') : 'empty'}. Click to cycle."
                    >
                      {#if v.kind === "bool" && v.value}
                        <svg viewBox="0 0 14 14" width="12" height="12" fill="none" aria-hidden="true">
                          <path d="M3 7.5l2.4 2.3L11 4.2" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>
                        </svg>
                      {:else if v.kind === "bool" && !v.value}
                        <svg viewBox="0 0 14 14" width="11" height="11" fill="none" aria-hidden="true">
                          <path d="M4 4l6 6M10 4l-6 6" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>
                        </svg>
                      {:else}
                        <span class="bool-null-glyph" aria-hidden="true">–</span>
                      {/if}
                    </button>
                  {:else if col.type === "date"}
                    <DatePicker
                      value={cellText(rowIdx, col.name)}
                      onChange={(next) => commitEdit(rowIdx, col.name, next, col.type)}
                    />
                  {:else if col.type === "list"}
                    {@const items = v.kind === "list" ? v.value.map(valueToDisplay) : []}
                    <div
                      class="list-cell"
                      onclick={(e) => focusListInput(e)}
                      onkeydown={(e) => {
                        // Forward keys from the cell shell to the inner input.
                        if (e.target === e.currentTarget) (e.currentTarget as HTMLElement).querySelector<HTMLInputElement>(".list-add")?.focus();
                      }}
                      role="presentation"
                    >
                      {#each items as item, idx (item + "::" + idx)}
                        <span class="list-chip" data-color={tagColor(item)}>
                          <span class="list-chip-text">{item}</span>
                          <button
                            type="button"
                            class="list-chip-x"
                            onclick={(e) => { e.stopPropagation(); removeListItem(rowIdx, col.name, idx); }}
                            aria-label="Remove {item}"
                            tabindex="-1"
                          >
                            <svg viewBox="0 0 12 12" width="9" height="9" fill="none">
                              <path d="M3 3l6 6M9 3l-6 6" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>
                            </svg>
                          </button>
                        </span>
                      {/each}
                      <input
                        type="text"
                        class="list-add"
                        placeholder={items.length === 0 ? "Add item…" : ""}
                        onkeydown={(e) => listKeyDown(e, rowIdx, col.name, items)}
                        onblur={(e) => listBlur(e, rowIdx, col.name)}
                      />
                    </div>
                  {:else}
                    <div class="cell-edit">
                      <!-- Display layer drives column width and cell height.
                           List cells are handled in their own branch above and
                           never reach this branch. -->
                      {#if empty}
                        <div class="display is-empty" aria-hidden="true">{emptyPlaceholderFor(col.type)}</div>
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
          {#if model.rows.length === 0 || model.rows.every((r) => r.pending_delete)}
            <tr class="empty-rows-tr" aria-hidden="true">
              <td class="empty-rows-cell" colspan={model.schema.columns.length + 2}>
                <span class="empty-rows-glyph">
                  <svg viewBox="0 0 24 24" width="20" height="20" fill="none">
                    <rect x="3" y="6" width="18" height="14" rx="2" stroke="currentColor" stroke-width="1.4"/>
                    <path d="M3 11h18" stroke="currentColor" stroke-width="1.4"/>
                  </svg>
                </span>
                <span class="empty-rows-text">
                  {#if model.rows.length === 0}
                    No rows yet — click <strong>+ {model.mode.kind === "folder" ? "File" : "Row"}</strong> below to add one.
                  {:else}
                    All rows are marked for deletion. Save All to commit, or restore one to keep editing.
                  {/if}
                </span>
              </td>
            </tr>
          {/if}
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

  <!-- Save toast — text is in an aria-live region so screen readers
       announce save success / failure / validation block. The role
       depends on severity (status for success, alert for errors). -->
  {#if toast}
    <div
      class="toast"
      class:success={toast.kind === "success"}
      class:error={toast.kind === "error"}
      role={toast.kind === "error" ? "alert" : "status"}
      aria-live={toast.kind === "error" ? "assertive" : "polite"}
      transition:fly={{ y: 12, duration: 220, easing: cubicOut }}
    >
      <span class="toast-icon" aria-hidden="true">
        {#if toast.kind === "success"}
          <svg viewBox="0 0 16 16" width="14" height="14" fill="none">
            <path d="M3.2 8.4l3 3 6.6-6.6" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
        {:else}
          <svg viewBox="0 0 16 16" width="14" height="14" fill="none">
            <path d="M8 1.5l7 12.5H1L8 1.5z" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round" />
            <path d="M8 6.5v3.5M8 12v.4" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
          </svg>
        {/if}
      </span>
      <span class="toast-text">{toast.text}</span>
      {#if toast.kind === "error" && saveFailures.length > 1}
        <button class="toast-action" onclick={openSaveFailures}>View details</button>
      {/if}
      <button class="toast-close" onclick={() => (toast = null)} aria-label="Dismiss">
        <svg viewBox="0 0 16 16" width="11" height="11" fill="none">
          <path d="M3 3l10 10M13 3L3 13" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
        </svg>
      </button>
    </div>
  {/if}

  {#if confirmCloseOpen}
    <div class="modal-backdrop" onclick={cancelClose} role="presentation"></div>
    <div class="modal modal-destructive" use:trapFocus role="alertdialog" aria-labelledby="discard-title" aria-modal="true">
      <header class="modal-destructive-head">
        <span class="modal-destructive-icon" aria-hidden="true">
          <svg viewBox="0 0 16 16" width="18" height="18" fill="none">
            <path d="M8 1.5l7 12.5H1L8 1.5z" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"/>
            <path d="M8 6.5v3.5M8 12v.4" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>
          </svg>
        </span>
        <h2 id="discard-title">Discard unsaved changes?</h2>
      </header>
      <div class="modal-destructive-body">
        <p>
          You have <strong>{dirtyBreakdown}</strong>. Closing now will discard them.
        </p>
        <p class="modal-destructive-meta">
          Cancel to return to the editor and Save All.
        </p>
      </div>
      <div class="modal-actions">
        <button type="button" class="btn-secondary" onclick={cancelClose}>Cancel</button>
        <button type="button" class="btn-danger" onclick={confirmDiscardAndClose}>Discard &amp; close</button>
      </div>
    </div>
  {/if}

  {#if saveFailuresOpen}
    <div class="modal-backdrop" onclick={closeSaveFailures} role="presentation"></div>
    <div class="modal modal-destructive modal-failures" use:trapFocus role="alertdialog" aria-labelledby="save-fail-title" aria-modal="true">
      <header class="modal-destructive-head">
        <span class="modal-destructive-icon" aria-hidden="true">
          <svg viewBox="0 0 16 16" width="18" height="18" fill="none">
            <path d="M8 1.5l7 12.5H1L8 1.5z" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"/>
            <path d="M8 6.5v3.5M8 12v.4" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>
          </svg>
        </span>
        <h2 id="save-fail-title">{saveFailures.length} files failed</h2>
      </header>
      <div class="modal-destructive-body">
        <p class="modal-destructive-meta">
          The following files were not written. The successful files are already on disk.
        </p>
        <ul class="failure-list">
          {#each saveFailures as f (f.path)}
            <li class="failure-item">
              <span class="failure-path" title={f.path}>{basename(f.path)}</span>
              <span class="failure-msg">{f.message}</span>
            </li>
          {/each}
        </ul>
      </div>
      <div class="modal-actions">
        <button type="button" class="btn-secondary" onclick={closeSaveFailures}>Close</button>
      </div>
    </div>
  {/if}

  {#if addColOpen}
    <div class="modal-backdrop" onclick={closeAddColumn} role="presentation"></div>
    <div class="modal" use:trapFocus role="dialog" aria-labelledby="add-col-title" aria-modal="true">
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
    <div class="modal" use:trapFocus role="dialog" aria-labelledby="edit-col-title" aria-modal="true">
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
    <div class="modal-backdrop" onclick={() => { addRowOpen = false; restoreFocus(); }} role="presentation"></div>
    <div class="modal" use:trapFocus role="dialog" aria-labelledby="add-row-title" aria-modal="true">
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
          <button type="button" class="btn-secondary" onclick={() => { addRowOpen = false; restoreFocus(); }}>Cancel</button>
          <button type="submit" class="btn-primary">Add</button>
        </div>
      </form>
    </div>
  {/if}

  {#if pendingDeleteCol !== null}
    <div class="modal-backdrop" onclick={cancelDeleteColumn} role="presentation"></div>
    <div class="modal modal-destructive" use:trapFocus role="alertdialog" aria-labelledby="del-col-title" aria-describedby="del-col-body" aria-modal="true">
      <header class="modal-destructive-head">
        <span class="modal-destructive-icon" aria-hidden="true">
          <svg viewBox="0 0 16 16" width="18" height="18" fill="none">
            <path d="M2.5 5.5h11M6 5.5V4a1 1 0 011-1h2a1 1 0 011 1v1.5M4 5.5l.6 8a1 1 0 001 .9h4.8a1 1 0 001-.9L12 5.5M7 8.5v3.5M9 8.5v3.5" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
        </span>
        <h2 id="del-col-title">Delete column</h2>
      </header>
      <div class="modal-destructive-body">
        <p id="del-col-body">
          The <code>{pendingDeleteCol}</code> field will be removed from every row.
        </p>
        <p class="modal-destructive-meta">
          Reversible until Save&nbsp;All — close the file without saving to undo.
        </p>
      </div>
      <div class="modal-actions">
        <button type="button" class="btn-secondary" onclick={cancelDeleteColumn}>Cancel</button>
        <button
          type="button"
          class="btn-danger"
          onclick={confirmDeleteColumn}
          bind:this={deleteConfirmButton}
        >
          <svg viewBox="0 0 16 16" width="13" height="13" fill="none" aria-hidden="true">
            <path d="M2.5 5.5h11M6 5.5V4a1 1 0 011-1h2a1 1 0 011 1v1.5M4 5.5l.6 8a1 1 0 001 .9h4.8a1 1 0 001-.9L12 5.5" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
          <span>Delete column</span>
        </button>
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
  /* Responsive collapse: under ~900px, the toolbar runs out of room for path
     breadcrumbs + meta + dirty pill + ghost buttons + Save All. Hide all but
     the last path segment so the file/folder name still anchors context. */
  @media (max-width: 900px) {
    .path-seg:not(.last) { display: none; }
    .path-sep { display: none; }
  }
  /* Under ~720px, hide the meta counters too — Save All + dirty pill stay. */
  @media (max-width: 720px) {
    .meta { display: none; }
    .meta-sep { display: none; }
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
    /* Tabular figures — counters don't shift width as digits roll. */
    font-feature-settings: "tnum" on, "lnum" on;
  }
  .meta-sep {
    width: 1px;
    height: 14px;
    background: var(--mt-divider);
  }

  .dirty-pill {
    display: inline-flex;
    align-items: center;
    gap: 6px;
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
    cursor: help;
  }
  .dirty-pill-label {
    color: var(--mt-fg-subtle);
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
  .modal.modal-destructive {
    width: min(460px, calc(100vw - 32px));
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
  /* Destructive variant: red strip + iconified header + softer paragraph
     hierarchy. The CTA on the right uses btn-danger which is already red. */
  .modal-destructive::before {
    background: linear-gradient(90deg, transparent 0%, var(--mt-error) 30%, var(--mt-error) 70%, transparent 100%);
    opacity: 0.7;
  }
  .modal-destructive-head {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 22px 26px 0;
  }
  .modal-destructive-icon {
    width: 36px;
    height: 36px;
    border-radius: 999px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: var(--mt-error-bg);
    color: var(--mt-error);
    flex-shrink: 0;
  }
  .modal-destructive-head h2 {
    margin: 0;
    font-family: var(--mt-font-display);
    font-weight: 600;
    font-size: 19px;
    line-height: 1.2;
    letter-spacing: -0.014em;
    color: var(--mt-fg);
  }
  .modal-destructive-body {
    padding: 14px 26px 4px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .modal-destructive-body p {
    margin: 0;
    font-size: 13px;
    line-height: 1.55;
    color: var(--mt-fg-muted);
  }
  .modal-destructive-body code {
    font-family: var(--mt-font-mono);
    font-size: 12px;
    background: var(--mt-surface-strong);
    padding: 1px 6px;
    border-radius: 3px;
    color: var(--mt-fg);
    margin: 0 2px;
  }
  .modal-destructive-meta {
    font-size: 12px !important;
    color: var(--mt-fg-subtle) !important;
  }
  /* The destructive modal isn't wrapped in a form with 26px padding, so the
     parent .modal-actions' negative-margin trick (designed for that wrapper)
     pushes content off the left edge. Reset the margins, give the action bar
     its own padding, and right-align the buttons. */
  .modal-destructive .modal-actions {
    margin: 0;
    padding: 14px 22px;
    border-top: 1px solid var(--mt-divider);
    justify-content: flex-end;
  }
  /* Hide the secondary "esc to close" caption inside the destructive variant.
     Destructive confirms shouldn't compete for attention with keyboard hints —
     the global Escape handler still works, and the visual is calmer without
     extra mono caps in the action bar. */
  .modal-destructive .modal-actions::before {
    content: none;
  }
  /* The leading SVG inside a destructive button gets its spacing from the
     button's flex `gap` — no manual margin/vertical-align needed. */
  .modal form,
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
    /* Quiet secondary caption — sits at the left of the action bar without
       imitating the buttons' visual weight. The kbd-shaped pill made the
       buttons appear vertically misaligned because it was a different
       height; plain text on the same flex baseline avoids that. */
    content: "esc to close";
    font-family: var(--mt-font-mono);
    font-size: 11px;
    letter-spacing: 0.04em;
    color: var(--mt-fg-subtle);
    margin-right: auto;
  }
  .btn-primary,
  .btn-secondary,
  .btn-danger {
    all: unset;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    /* Fixed height so all variants align on the same baseline regardless
       of whether they contain a leading icon or just a label. */
    height: 32px;
    padding: 0 14px;
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
  /* Pencil glyph next to the column name. Invisible at rest so the header
     reads cleanly; reveals on hover (paired with the cursor:pointer above)
     to signal "click to edit." Sits between the name and the delete x. */
  .col-edit-hint {
    display: inline-flex;
    align-items: center;
    margin-left: 5px;
    color: var(--mt-fg-subtle);
    opacity: 0;
    transition: opacity 140ms ease;
    pointer-events: none;
    vertical-align: middle;
  }
  thead th.col-head:hover .col-edit-hint,
  thead th.col-head:focus-within .col-edit-hint {
    opacity: 0.8;
  }
  /* Per-column delete (×) — only visible on header hover. */
  thead th.col-head {
    position: sticky;
    top: 0;
    padding-right: 28px;
  }
  /* Hit area is 24×24 (visible 18×18 glyph centered inside). At 18×18 the
     button met neither WCAG 2.5.5 (44×44 ideal) nor common desktop minimum
     (24×24) — easy to miss with the cursor. The glyph stays visually small
     so the header isn't cluttered; only the click area grows. */
  .col-delete {
    all: unset;
    position: absolute;
    top: 4px;
    right: 4px;
    width: 24px;
    height: 24px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: 4px;
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

  /* Empty-rows tbody placeholder shown when every row is gone or pending-delete. */
  tr.empty-rows-tr td {
    border: none;
    background: var(--mt-page-bg);
    padding: 36px 24px;
  }
  td.empty-rows-cell {
    text-align: center;
    color: var(--mt-fg-muted);
    font-size: 13px;
    line-height: 1.6;
  }
  .empty-rows-glyph {
    display: block;
    margin: 0 auto 10px;
    color: var(--mt-fg-subtle);
  }
  .empty-rows-text strong {
    color: var(--mt-fg);
    font-weight: 500;
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

  /* Per-row delete: 24×24 hit area for the same reason as .col-delete. */
  .row-delete {
    all: unset;
    position: absolute;
    top: 4px;
    right: 4px;
    width: 24px;
    height: 24px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: 4px;
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
  /* Inline-editable filename input (folder mode only). Looks like the row-num
     when at rest, becomes a focused input on click/Tab. Pending rename gets a
     soft accent ring so it's visually obvious before Save All. */
  .row-name-input {
    all: unset;
    display: inline-block;
    margin-left: 8px;
    padding: 1px 4px;
    color: var(--mt-fg);
    font: inherit;
    font-family: var(--mt-font-mono);
    font-size: 12px;
    vertical-align: top;
    max-width: 220px;
    border-radius: 3px;
    border: 1px solid transparent;
    transition: border-color 120ms ease, background 120ms ease;
    cursor: text;
  }
  td.row-head:hover .row-name-input {
    border-color: var(--mt-border-strong);
    background: var(--mt-elevated);
  }
  .row-name-input:focus-visible {
    border-color: var(--mt-accent);
    background: var(--mt-elevated);
    box-shadow: 0 0 0 2px var(--mt-accent-soft);
  }
  td.row-head.renamed .row-name-input {
    color: var(--mt-accent);
    border-color: color-mix(in srgb, var(--mt-accent) 35%, transparent);
    background: var(--mt-accent-soft);
  }
  td.row-head.invalid {
    box-shadow: inset 0 0 0 1px var(--mt-error);
    background: color-mix(in srgb, var(--mt-error-bg) 70%, var(--mt-surface)) !important;
  }
  td.row-head.invalid .row-name-input {
    color: var(--mt-error);
    border-color: var(--mt-error);
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
  /* Parse-error rows are read-only and shown with a diagonal hatch over the
     red wash so they're unmistakably broken-but-shown rather than just
     "another row with red bg." The row-head gets a small ⚠ badge prefix
     via .row-head[data-row-error]. */
  tbody tr.errored td {
    background:
      repeating-linear-gradient(
        135deg,
        transparent 0,
        transparent 6px,
        color-mix(in srgb, var(--mt-error) 8%, transparent) 6px,
        color-mix(in srgb, var(--mt-error) 8%, transparent) 7px
      ),
      var(--mt-error-bg);
  }
  tbody tr.errored td.row-head::before {
    content: "⚠";
    color: var(--mt-error);
    font-size: 12px;
    margin-right: 6px;
    vertical-align: -1px;
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

  /* Three-state boolean cell. The previous binary checkbox conflated null
     with false; this version cycles null → true → false → null on click,
     showing a typed glyph for each state. Stays a single-tap target so
     it's still keyboard-friendly via Enter/Space. */
  .bool-cell {
    all: unset;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
    min-height: 36px;
    color: var(--mt-fg-subtle);
    font-family: var(--mt-font-mono);
    font-size: 13px;
    transition: background 120ms ease, color 120ms ease;
  }
  .bool-cell:hover {
    background: var(--mt-hover);
  }
  .bool-cell:focus-visible {
    outline: 2px solid var(--mt-accent);
    outline-offset: -2px;
  }
  .bool-cell.bool-true {
    color: var(--mt-success);
  }
  .bool-cell.bool-false {
    color: var(--mt-fg-muted);
  }
  .bool-cell.bool-null .bool-null-glyph {
    font-style: italic;
    color: var(--mt-fg-subtle);
  }

  /* List cell — chip pills with × removal + trailing add-input. Replaces
     the previous "comma-separated textarea" pattern. The cell is one big
     click target that focuses the trailing input. Backspace on an empty
     input removes the last chip. */
  .list-cell {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    padding: 6px 10px;
    min-height: 36px;
    align-items: center;
    cursor: text;
  }
  .list-chip {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    padding: 1px 4px 1px 7px;
    border-radius: 3px;
    font-size: 11.5px;
    line-height: 17px;
    white-space: nowrap;
    flex-shrink: 0;
  }
  .list-chip-x {
    all: unset;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 14px;
    border-radius: 2px;
    color: inherit;
    opacity: 0.5;
    transition: opacity 100ms ease, background 100ms ease;
  }
  .list-chip:hover .list-chip-x,
  .list-chip-x:focus-visible {
    opacity: 1;
  }
  .list-chip-x:hover {
    background: rgba(0, 0, 0, 0.12);
  }
  .list-add {
    all: unset;
    flex: 1;
    min-width: 60px;
    font: inherit;
    font-size: 13px;
    color: var(--mt-fg);
    cursor: text;
    padding: 1px 2px;
  }
  .list-add::placeholder {
    color: var(--mt-fg-subtle);
  }

  .list-chip[data-color="blue"] {
    background: var(--mt-tag-blue);
    color: var(--mt-tag-blue-fg);
  }
  .list-chip[data-color="green"] {
    background: var(--mt-tag-green);
    color: var(--mt-tag-green-fg);
  }
  .list-chip[data-color="yellow"] {
    background: var(--mt-tag-yellow);
    color: var(--mt-tag-yellow-fg);
  }
  .list-chip[data-color="orange"] {
    background: var(--mt-tag-orange);
    color: var(--mt-tag-orange-fg);
  }
  .list-chip[data-color="red"] {
    background: var(--mt-tag-red);
    color: var(--mt-tag-red-fg);
  }
  .list-chip[data-color="purple"] {
    background: var(--mt-tag-purple);
    color: var(--mt-tag-purple-fg);
  }
  .list-chip[data-color="pink"] {
    background: var(--mt-tag-pink);
    color: var(--mt-tag-pink-fg);
  }
  .list-chip[data-color="gray"] {
    background: var(--mt-tag-gray);
    color: var(--mt-tag-gray-fg);
  }

  /* Dirty marker — a 2px left-edge bar in the warning color, much more
     legible in a dense table than the previous 6px corner dot. Notion
     and Linear use the same pattern. */
  .dirty-mark {
    position: absolute;
    top: 0;
    left: 0;
    bottom: 0;
    width: 2px;
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
  .toast-action {
    all: unset;
    cursor: pointer;
    flex-shrink: 0;
    padding: 3px 10px;
    border-radius: 4px;
    color: #fff;
    font-size: 12px;
    font-weight: 500;
    background: rgba(255, 255, 255, 0.18);
    transition: background 120ms ease;
  }
  .toast-action:hover {
    background: rgba(255, 255, 255, 0.28);
  }
  .toast-action:focus-visible {
    outline: 2px solid rgba(255, 255, 255, 0.6);
    outline-offset: 2px;
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

  /* Save-failures detail modal */
  .modal-failures {
    width: min(560px, calc(100vw - 32px));
  }
  .failure-list {
    list-style: none;
    margin: 8px 0 0;
    padding: 0;
    max-height: 300px;
    overflow-y: auto;
    border: 1px solid var(--mt-divider);
    border-radius: 6px;
    background: var(--mt-surface);
  }
  .failure-item {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 10px 12px;
    border-bottom: 1px solid var(--mt-divider);
  }
  .failure-item:last-child {
    border-bottom: none;
  }
  .failure-path {
    font-family: var(--mt-font-mono);
    font-size: 12.5px;
    color: var(--mt-fg);
    font-weight: 500;
  }
  .failure-msg {
    font-size: 12px;
    line-height: 1.4;
    color: var(--mt-fg-muted);
    white-space: pre-wrap;
    word-break: break-word;
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
