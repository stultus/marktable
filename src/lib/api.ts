import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

export type FieldType = "text" | "number" | "boolean" | "date" | "list" | "raw";

export type Value =
  | { kind: "null" }
  | { kind: "bool"; value: boolean }
  | { kind: "number"; value: number }
  | { kind: "string"; value: string }
  | { kind: "date"; value: string }
  | { kind: "list"; value: Value[] }
  | { kind: "raw"; value: string };

export interface Column {
  name: string;
  type: FieldType;
}

export interface Schema {
  columns: Column[];
}

export interface Record {
  fields: { [key: string]: Value };
}

export type RowSource =
  | { kind: "file"; path: string; original_text: string }
  | { kind: "inline"; index: number };

export interface Row {
  id: string;
  source: RowSource;
  record: Record;
  parse_error: string | null;
}

export type TableMode =
  | { kind: "folder"; path: string }
  | { kind: "json_file"; path: string; original_text: string }
  | { kind: "yaml_file"; path: string; original_text: string };

export type Warning =
  | { kind: "empty_columns"; names: string[] }
  | { kind: "unparseable_file"; path: string; message: string };

export interface TableModel {
  mode: TableMode;
  schema: Schema;
  rows: Row[];
  warnings: Warning[];
}

export interface SaveResult {
  written: string[];
  failures: { path: string; message: string }[];
}

export async function pickFolder(): Promise<string | null> {
  const result = await open({ directory: true, multiple: false });
  return typeof result === "string" ? result : null;
}

export async function pickFile(): Promise<string | null> {
  const result = await open({
    multiple: false,
    filters: [{ name: "Structured data", extensions: ["json", "yaml", "yml"] }],
  });
  return typeof result === "string" ? result : null;
}

export async function openFolder(path: string): Promise<TableModel> {
  return await invoke<TableModel>("open_folder", { path });
}

export async function openFile(path: string): Promise<TableModel> {
  return await invoke<TableModel>("open_file", { path });
}

export async function saveAll(table: TableModel): Promise<SaveResult> {
  return await invoke<SaveResult>("save_all", { table });
}

export function valueToDisplay(v: Value): string {
  switch (v.kind) {
    case "null":
      return "";
    case "bool":
      return v.value ? "true" : "false";
    case "number":
      return String(v.value);
    case "string":
    case "date":
    case "raw":
      return v.value;
    case "list":
      return v.value.map(valueToDisplay).join(", ");
  }
}

export function displayToValue(text: string, type: FieldType): Value {
  if (text === "") return { kind: "null" };
  switch (type) {
    case "boolean":
      return { kind: "bool", value: text.toLowerCase() === "true" };
    case "number": {
      const n = Number(text);
      if (Number.isFinite(n)) return { kind: "number", value: n };
      return { kind: "string", value: text };
    }
    case "date":
      return { kind: "date", value: text };
    case "list":
      return {
        kind: "list",
        value: text
          .split(",")
          .map((s) => s.trim())
          .filter((s) => s.length > 0)
          .map((s) => ({ kind: "string", value: s }) as Value),
      };
    case "raw":
      return { kind: "raw", value: text };
    case "text":
    default:
      return { kind: "string", value: text };
  }
}

export function basename(path: string): string {
  const i = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return i >= 0 ? path.slice(i + 1) : path;
}
