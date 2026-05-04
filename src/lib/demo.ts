// TEMPORARY demo harness — lets us drive the TableView with hardcoded data
// without going through Tauri commands, so we can render the UI in plain
// Chrome (headless or otherwise) for screenshotting and CSS verification.
//
// Triggered by `?demo=json` or `?demo=yaml` URL params. Remove once the UI
// is settled.

import type { TableModel, Value } from "./api";

const s = (value: string): Value => ({ kind: "string", value });
const n = (value: number): Value => ({ kind: "number", value });
const b = (value: boolean): Value => ({ kind: "bool", value });
const d = (value: string): Value => ({ kind: "date", value });
const list = (...items: string[]): Value => ({
  kind: "list",
  value: items.map(s),
});
const NULL: Value = { kind: "null" };

export function demoJson(): TableModel {
  return {
    mode: {
      kind: "json_file",
      path: "/Users/stultus/Documents/Projects/Personal/listty/sample-array.json",
      original_text: "",
    },
    schema: {
      columns: [
        { name: "title", type: "text" },
        { name: "author", type: "text" },
        { name: "year", type: "number" },
        { name: "rating", type: "number" },
        { name: "read", type: "boolean" },
        { name: "started", type: "date" },
        { name: "tags", type: "list" },
        { name: "notes", type: "text" },
      ],
    },
    rows: [
      {
        id: "row-0",
        source: { kind: "inline", index: 0 },
        parse_error: null,
        record: {
          fields: {
            title: s("The Pragmatic Programmer"),
            author: s("Andrew Hunt, David Thomas"),
            year: n(1999),
            rating: n(5),
            read: b(true),
            started: d("2024-03-12"),
            tags: list("software", "career", "classic"),
            notes: s("Reread every couple of years."),
          },
        },
      },
      {
        id: "row-1",
        source: { kind: "inline", index: 1 },
        parse_error: null,
        record: {
          fields: {
            title: s("Designing Data-Intensive Applications"),
            author: s("Martin Kleppmann"),
            year: n(2017),
            rating: n(5),
            read: b(true),
            started: d("2025-01-04"),
            tags: list("distributed-systems", "databases"),
            notes: s("The chapter on consensus is worth the price alone."),
          },
        },
      },
      {
        id: "row-2",
        source: { kind: "inline", index: 2 },
        parse_error: null,
        record: {
          fields: {
            title: s("A Philosophy of Software Design"),
            author: s("John Ousterhout"),
            year: n(2018),
            rating: n(4),
            read: b(true),
            started: d("2024-09-22"),
            tags: list("software", "design"),
            notes: s(""),
          },
        },
      },
      {
        id: "row-3",
        source: { kind: "inline", index: 3 },
        parse_error: null,
        record: {
          fields: {
            title: s("Crafting Interpreters"),
            author: s("Robert Nystrom"),
            year: n(2021),
            rating: n(5),
            read: b(false),
            started: d("2026-02-18"),
            tags: list("compilers", "languages"),
            notes: s("Halfway through the tree-walker."),
          },
        },
      },
      {
        id: "row-4",
        source: { kind: "inline", index: 4 },
        parse_error: null,
        record: {
          fields: {
            title: s("The Annotated Turing"),
            author: s("Charles Petzold"),
            year: n(2008),
            rating: NULL,
            read: b(false),
            started: NULL,
            tags: list("theory", "history"),
            notes: NULL,
          },
        },
      },
    ],
    warnings: [],
  };
}

export function demoYaml(): TableModel {
  const m = demoJson();
  return {
    ...m,
    mode: {
      kind: "yaml_file",
      path: "/Users/stultus/Documents/Projects/Personal/listty/sample-list.yaml",
      original_text: "",
    },
  };
}
