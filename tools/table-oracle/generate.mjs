// Generates tests/fixtures/table-oracle.json: what @tanstack/table-core
// 9.3.1, the published package, computes for sorting, column sizing and
// column resizing over seeded tables and seeded operations. The Rust port in
// src/table must reproduce every snapshot exactly (tests/table_oracle.rs).
//
//     cd reference && npm ci && cd ..
//     node tools/table-oracle/generate.mjs tests/fixtures/table-oracle.json
//
// Node has no requestAnimationFrame, so table-core applies each resize move
// at once (its documented fallback); the frame-coalesced path is the view's
// and is checked against Chromium by the fidelity runner instead.
//
// Values that JSON can't carry are tagged: {"$": "undefined" | "NaN" |
// "Infinity" | "-Infinity" | "-0"} and {"$date": <ms>} for a Date.

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");
const modules = join(root, "reference", "node_modules", "@tanstack");
const PACKAGE = JSON.parse(readFileSync(join(modules, "table-core", "package.json"), "utf8"));
if (PACKAGE.version !== "9.3.1") {
  console.error(`expected @tanstack/table-core 9.3.1, found ${PACKAGE.version}: run npm ci in reference`);
  process.exit(1);
}
const STORE = JSON.parse(readFileSync(join(modules, "store", "package.json"), "utf8")).version;
const core = await import(pathToFileURL(join(modules, "table-core", "dist", "index.js")).href);
const { storeReactivityBindings } = await import(
  pathToFileURL(join(modules, "table-core", "dist", "store-reactivity-bindings.js")).href
);

const [output] = process.argv.slice(2);
if (!output) {
  console.error("usage: node tools/table-oracle/generate.mjs OUTPUT");
  process.exit(2);
}

// ---------------------------------------------------------------- values

function encode(value) {
  if (value === undefined) return { $: "undefined" };
  if (typeof value === "number") {
    if (Number.isNaN(value)) return { $: "NaN" };
    if (value === Infinity) return { $: "Infinity" };
    if (value === -Infinity) return { $: "-Infinity" };
    if (Object.is(value, -0)) return { $: "-0" };
    return value;
  }
  if (value instanceof Date) return { $date: encode(value.getTime()) };
  if (Array.isArray(value)) return value.map(encode);
  if (value && typeof value === "object") {
    return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, encode(v)]));
  }
  return value;
}

function decode(value) {
  if (Array.isArray(value)) return value.map(decode);
  if (value && typeof value === "object") {
    if ("$" in value) {
      return { undefined: undefined, NaN: NaN, Infinity: Infinity, "-Infinity": -Infinity, "-0": -0 }[value.$];
    }
    if ("$date" in value) return new Date(decode(value.$date));
    return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, decode(v)]));
  }
  return value;
}

// ------------------------------------------------------------- the table

// The custom sort function of table-core's own vanilla sorting example.
const STATUS_ORDER = ["single", "complicated", "relationship"];
const CUSTOM_SORT_FNS = {
  statusOrder: (rowA, rowB, columnId) =>
    STATUS_ORDER.indexOf(rowA.getValue(columnId)) - STATUS_ORDER.indexOf(rowB.getValue(columnId)),
};

function columnDef(spec) {
  const def = {};
  for (const [key, value] of Object.entries(spec)) {
    if (key === "columns") def.columns = value.map(columnDef);
    else if (key === "accessorFn") def.accessorFn = (row) => row[value];
    else if (key === "sortFn" && value.startsWith("custom:")) def.sortFn = CUSTOM_SORT_FNS[value.slice(7)];
    else def[key] = value;
  }
  return def;
}

function build(spec) {
  const registered = spec.sortFns ?? Object.keys(core.sortFns);
  const features = core.tableFeatures({
    rowSortingFeature: core.rowSortingFeature,
    columnSizingFeature: core.columnSizingFeature,
    columnResizingFeature: core.columnResizingFeature,
    coreReactivityFeature: storeReactivityBindings(),
    sortedRowModel: core.createSortedRowModel(),
    sortFns: Object.fromEntries(registered.map((name) => [name, core.sortFns[name]])),
  });
  const options = { ...spec.options };
  if (options.defaultColumn) options.defaultColumn = columnDef(options.defaultColumn);
  if (options.getRowId) {
    const key = options.getRowId;
    options.getRowId = (row) => row[key];
  }
  if (options.getSubRows) {
    const key = options.getSubRows;
    options.getSubRows = (row) => row[key];
  }
  if (spec.initialState) {
    options.initialState = {
      ...spec.initialState,
      ...(spec.initialState.sorting && {
        sorting: spec.initialState.sorting.map(([id, desc]) => ({ id, desc })),
      }),
      ...(spec.initialState.columnSizing && {
        columnSizing: Object.fromEntries(spec.initialState.columnSizing),
      }),
    };
  }
  return core.constructTable({
    features,
    data: decode(spec.data),
    columns: spec.columns.map(columnDef),
    ...options,
  });
}

function sortFnName(fn) {
  for (const [name, builtin] of Object.entries(core.sortFns)) if (fn === builtin) return name;
  for (const [name, custom] of Object.entries(CUSTOM_SORT_FNS)) if (fn === custom) return `custom:${name}`;
  return "unknown";
}

function snapshot(table) {
  const model = table.getSortedRowModel();
  const groups = table.getHeaderGroups();
  return {
    sorting: table.atoms.sorting.get().map((s) => [s.id, s.desc]),
    rows: model.rows.map((row) => row.id),
    flat: model.flatRows.map((row) => row.id),
    columns: table.getAllFlatColumns().map((c) => [
      c.id,
      c.depth,
      c.getCanSort(),
      c.getCanMultiSort(),
      c.getIsSorted(),
      c.getSortIndex(),
      c.getNextSortingOrder(),
      c.getFirstSortDir(),
      c.getAutoSortDir(),
      sortFnName(c.getSortFn()),
      encode(c.getSize()),
      encode(c.getStart()),
      encode(c.getAfter()),
      c.getCanResize(),
      c.getIsResizing(),
    ]),
    headerGroups: groups.map((g) => [
      g.id,
      g.depth,
      g.headers.map((h) => [
        h.id,
        h.column.id,
        h.index,
        h.depth,
        h.colSpan,
        h.rowSpan,
        h.isPlaceholder,
        h.placeholderId ?? null,
        encode(h.getSize()),
        encode(h.getStart()),
        h.subHeaders.map((s) => s.id),
        h.getLeafHeaders().map((l) => l.id),
      ]),
    ]),
    totalSize: encode(table.getTotalSize()),
    columnSizing: Object.entries(table.atoms.columnSizing.get()).map(([k, v]) => [k, encode(v)]),
    columnResizing: encode(table.atoms.columnResizing.get()),
  };
}

function headerById(table, id) {
  for (const group of table.getHeaderGroups()) for (const h of group.headers) if (h.id === id) return h;
  throw new Error(`no header ${id}`);
}

function mouse(type, x) {
  return Object.assign(new Event(type), { clientX: x });
}

function touch(type, xs) {
  return Object.assign(new Event(type), { touches: xs.map((clientX) => ({ clientX })) });
}

function apply(table, doc, step) {
  switch (step.op) {
    case "toggle":
      table.getColumn(step.column).toggleSorting(step.desc, step.multi);
      break;
    case "click":
      table.getColumn(step.column).getToggleSortingHandler()({ shiftKey: step.shift });
      break;
    case "clear":
      table.getColumn(step.column).clearSorting();
      break;
    case "setSorting":
      table.setSorting(step.sorting.map(([id, desc]) => ({ id, desc })));
      break;
    case "resetSorting":
      table.resetSorting(step.defaultState);
      break;
    case "resizeStart": {
      const event = step.touches ? touch("touchstart", step.touches) : mouse("mousedown", step.x);
      headerById(table, step.header).getResizeHandler(doc)(event);
      break;
    }
    case "move":
      doc.dispatchEvent(step.touch ? touch("touchmove", [step.x]) : mouse("mousemove", step.x));
      break;
    case "end":
      if (step.cancel) doc.dispatchEvent(new Event("touchcancel"));
      else if (step.touch) doc.dispatchEvent(touch("touchend", step.x === undefined ? [] : [step.x]));
      else doc.dispatchEvent(mouse("mouseup", step.x));
      break;
    case "setColumnSizing":
      table.setColumnSizing(Object.fromEntries(step.sizing));
      break;
    case "resetSize":
      table.getColumn(step.column).resetSize();
      break;
    case "resetColumnSizing":
      table.resetColumnSizing(step.defaultState);
      break;
    default:
      throw new Error(`unknown op ${step.op}`);
  }
}

function run(spec) {
  const table = build(spec);
  const doc = new EventTarget();
  const snapshots = [snapshot(table)];
  for (const step of spec.steps) {
    apply(table, doc, step);
    snapshots.push(snapshot(table));
  }
  return snapshots;
}

// ---------------------------------------------------------- seeded cases

function mulberry32(seed) {
  return () => {
    seed = (seed + 0x6d2b79f5) | 0;
    let t = seed;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function generator(seed) {
  const random = mulberry32(seed);
  const int = (n) => Math.floor(random() * n);
  const pick = (list) => list[int(list.length)];
  const chance = (p) => random() < p;
  return { random, int, pick, chance };
}

const WORDS = ["apple", "Apple", "banana", "cherry", "Échelle", "echo", "zeta", "ΣΟΦΟΣ", "σοφος", "İstanbul", "a😀", "aﬁ", "", " pad", "Z"];
const STATUSES = ["single", "complicated", "relationship", "unknown"];

function valueOf(g, kind) {
  switch (kind) {
    case "word":
      return g.pick(WORDS);
    case "alnum": {
      const zeros = g.chance(0.2) ? "0".repeat(g.int(3)) : "";
      const digits = g.chance(0.05) ? "12345678901234567" + g.int(10) : String(g.int(120));
      return g.pick(["item", "Item", "v", "", "x-"]) + zeros + digits + g.pick(["", "b", "-2", ".5"]);
    }
    case "int":
      return g.int(6) - 2;
    case "float":
      if (g.chance(0.1)) return g.pick([NaN, Infinity, -Infinity, -0, 0]);
      return Math.round((g.random() - 0.4) * 1e4) / 100;
    case "date":
      if (g.chance(0.05)) return new Date(NaN);
      return new Date(Date.UTC(2020, 0, 1) + g.int(2000) * 86400000);
    case "bool":
      return g.chance(0.5);
    case "status":
      return g.pick(STATUSES);
    case "numstring":
      return g.pick(["12", " 12 ", "3", "0x1F", "1e2", "", "abc", "-4.5", "Infinity", "7"]);
    case "mixed":
      return valueOf(g, g.pick(["word", "alnum", "int", "float", "date", "bool", "numstring", "nullish"]));
    case "nullish":
      return g.pick([null, undefined]);
    default:
      throw new Error(kind);
  }
}

const KINDS = ["word", "alnum", "int", "float", "date", "bool", "status", "numstring", "mixed"];
const SORT_FNS = ["auto", "auto", "auto", "alphanumeric", "alphanumericCaseSensitive", "text", "textCaseSensitive", "datetime", "basic", "custom:statusOrder", "missing"];

function randomCase(seed, { rows: rowCount, steps: stepCount, sortOnly }) {
  const g = generator(seed);
  const leaves = [];
  let nextLeaf = 0;
  const leafSpec = () => {
    const key = `c${nextLeaf++}`;
    const kind = g.pick(KINDS);
    const spec = {};
    if (g.chance(0.15)) {
      spec.accessorKey = `info.${key}`;
      leaves.push({ key, kind, deep: true });
    } else if (g.chance(0.1)) {
      spec.id = key;
      spec.accessorFn = key;
      leaves.push({ key, kind });
    } else {
      spec.accessorKey = key;
      leaves.push({ key, kind });
    }
    if (g.chance(0.2)) spec.size = g.pick([40, 80, 100.5, 0, 33.333]);
    if (g.chance(0.1)) spec.minSize = g.pick([10, 50, 120]);
    if (g.chance(0.1)) spec.maxSize = g.pick([60, 200, 90.25]);
    if (g.chance(0.2)) spec.sortFn = g.pick(SORT_FNS);
    if (g.chance(0.15)) spec.sortUndefined = g.pick([false, -1, 1, "first", "last"]);
    if (g.chance(0.1)) spec.invertSorting = true;
    if (g.chance(0.1)) spec.sortDescFirst = g.chance(0.5);
    if (g.chance(0.08)) spec.enableSorting = false;
    if (g.chance(0.08)) spec.enableMultiSort = g.chance(0.5);
    if (g.chance(0.08)) spec.enableResizing = false;
    return spec;
  };
  const columnSpecs = (depth) => {
    const list = [];
    const count = 1 + g.int(depth === 0 ? 4 : 3);
    for (let i = 0; i < count; i++) {
      if (depth < 2 && g.chance(0.25)) {
        const group = { header: `G${depth}_${i}_${seed % 97}`, columns: columnSpecs(depth + 1) };
        if (g.chance(0.3)) group.size = g.pick([60, 300]);
        if (g.chance(0.1)) group.enableResizing = false;
        list.push(group);
      } else {
        list.push(leafSpec());
      }
    }
    return list;
  };
  const columns = columnSpecs(0);
  if (g.chance(0.1)) columns.push({ id: "display", header: "#" });

  const options = {};
  if (g.chance(0.1)) options.enableSorting = false;
  if (g.chance(0.15)) options.enableMultiSort = g.chance(0.5);
  if (g.chance(0.2)) options.enableSortingRemoval = g.chance(0.5);
  if (g.chance(0.15)) options.enableMultiRemove = g.chance(0.5);
  if (g.chance(0.2)) options.maxMultiSortColCount = g.pick([0, 1, 2, 2.5]);
  if (g.chance(0.15)) options.sortDescFirst = g.chance(0.5);
  if (g.chance(0.3)) options.columnResizeMode = "onChange";
  if (g.chance(0.2)) options.columnResizeDirection = "rtl";
  if (g.chance(0.08)) options.enableColumnResizing = false;
  if (g.chance(0.15)) options.defaultColumn = { size: g.pick([90, 120.5]), minSize: g.pick([30, 5]) };
  const subRows = g.chance(0.2);
  if (subRows) options.getSubRows = "children";
  const ownIds = g.chance(0.1);
  if (ownIds) options.getRowId = "key";

  let rowNumber = 0;
  const row = (depth) => {
    const data = {};
    const info = {};
    for (const leaf of leaves) {
      if (g.chance(0.08)) continue; // absent: undefined
      const value = valueOf(g, leaf.kind);
      if (leaf.deep) info[leaf.key] = value;
      else data[leaf.key] = value;
    }
    if (!g.chance(0.1)) data.info = info;
    if (ownIds) data.key = `r${rowNumber++}`;
    if (subRows && depth < 2 && g.chance(0.3)) {
      data.children = Array.from({ length: g.int(4) }, () => row(depth + 1));
    }
    return data;
  };
  const data = Array.from({ length: rowCount }, () => row(0));

  const spec = { seed, columns, options, data: encode(data), steps: [] };
  if (g.chance(0.2)) spec.sortFns = ["text", "basic"];
  const leafIds = leaves.map((leaf) => (leaf.deep ? `info_${leaf.key}` : leaf.key));
  if (g.chance(0.15)) spec.initialState = { sorting: [[g.pick(leafIds), g.chance(0.5)]] };
  if (g.chance(0.15)) {
    spec.initialState = { ...spec.initialState, columnSizing: [[g.pick(leafIds), g.pick([70, 12.5])]] };
  }

  // Steps run against a live table so that resize steps name real headers.
  const table = build(spec);
  const doc = new EventTarget();
  let resizing = null;
  for (let i = 0; i < stepCount; i++) {
    let step;
    const roll = g.random();
    if (resizing) {
      const x = resizing.x + Math.round((g.random() - 0.5) * 4000) / 10;
      if (g.chance(0.7)) step = { op: "move", x, ...(resizing.touch && { touch: true }) };
      else if (resizing.touch) step = g.chance(0.2) ? { op: "end", cancel: true } : { op: "end", touch: true, ...(g.chance(0.5) && { x }) };
      else step = { op: "end", x };
      if (step.op === "end") resizing = null;
    } else if (sortOnly || roll < 0.55) {
      const column = g.pick(leafIds);
      const kind = g.random();
      if (kind < 0.6) step = { op: "click", column, shift: g.chance(0.4) };
      else if (kind < 0.85) {
        step = { op: "toggle", column };
        if (g.chance(0.4)) step.desc = g.chance(0.5);
        if (g.chance(0.5)) step.multi = g.chance(0.7);
      } else if (kind < 0.93) step = { op: "clear", column };
      else if (kind < 0.97) step = { op: "setSorting", sorting: [[g.pick(leafIds), g.chance(0.5)], [g.pick(leafIds), g.chance(0.5)]] };
      else step = { op: "resetSorting", defaultState: g.chance(0.5) };
    } else if (roll < 0.85) {
      const headers = table.getHeaderGroups().flatMap((group) => group.headers);
      const header = g.pick(headers);
      const x = Math.round(g.random() * 8000) / 10;
      if (g.chance(0.25)) {
        const touches = g.chance(0.15) ? [x, x + 10] : [x + 0.5];
        step = { op: "resizeStart", header: header.id, touches };
        if (touches.length === 1 && header.column.getCanResize()) resizing = { x, touch: true };
      } else {
        step = { op: "resizeStart", header: header.id, x };
        if (header.column.getCanResize()) resizing = { x };
      }
    } else if (roll < 0.92) {
      step = { op: "setColumnSizing", sizing: [[g.pick(leafIds), g.pick([0, 25, 150.75, -10])]] };
    } else if (roll < 0.97) {
      step = { op: "resetSize", column: g.pick(leafIds) };
    } else {
      step = { op: "resetColumnSizing", defaultState: g.chance(0.5) };
    }
    apply(table, doc, step);
    spec.steps.push(step);
  }
  if (resizing) {
    const step = resizing.touch ? { op: "end", touch: true } : { op: "end", x: resizing.x };
    apply(table, doc, step);
    spec.steps.push(step);
  }
  return spec;
}

// ------------------------------------------------------------ hand cases

const HAND = [
  {
    name: "vanilla-sorting-example",
    columns: [
      { id: "rowNumber", header: "#" },
      { accessorKey: "firstName" },
      { id: "lastName", accessorFn: "lastName", header: "Last Name" },
      { accessorKey: "age", sortFn: "basic" },
      { accessorKey: "visits", sortUndefined: "last", sortDescFirst: false },
      { accessorKey: "status", sortFn: "custom:statusOrder" },
      { accessorKey: "rank", invertSorting: true },
      { accessorKey: "createdAt", sortFn: "datetime" },
    ],
    sortFns: ["alphanumeric", "datetime", "text"],
    data: [
      { firstName: "tanner", lastName: "linsley", age: 33, visits: 100, status: "single", rank: 2, createdAt: { $date: 1600000000000 } },
      { firstName: "Tandy", lastName: "miller", age: 40, status: "complicated", rank: 1, createdAt: { $date: 1500000000000 } },
      { firstName: "joe", lastName: "dirte", age: 45, visits: 20, status: "relationship", rank: 3, createdAt: { $date: 1700000000000 } },
      { firstName: "item10", lastName: "Linsley", age: 33, visits: 20, status: "single", rank: 2, createdAt: { $date: 1600000000000 } },
      { firstName: "item9", lastName: "b", age: { $: "NaN" }, visits: 5, status: "single", rank: 0, createdAt: { $date: { $: "NaN" } } },
    ],
    steps: [
      { op: "click", column: "firstName", shift: false },
      { op: "click", column: "firstName", shift: false },
      { op: "click", column: "firstName", shift: false },
      { op: "click", column: "age", shift: false },
      { op: "click", column: "lastName", shift: true },
      { op: "click", column: "visits", shift: true },
      { op: "click", column: "visits", shift: true },
      { op: "click", column: "visits", shift: true },
      { op: "click", column: "status", shift: false },
      { op: "click", column: "rank", shift: false },
      { op: "click", column: "createdAt", shift: false },
      { op: "click", column: "rowNumber", shift: false },
      { op: "toggle", column: "age", desc: true, multi: true },
      { op: "clear", column: "createdAt" },
    ],
  },
  {
    name: "column-resizing-example",
    columns: [
      { header: "Name", columns: [{ accessorKey: "firstName" }, { id: "lastName", accessorFn: "lastName", header: "Last Name" }] },
      { header: "Info", columns: [{ accessorKey: "age", size: 50 }, { header: "More Info", columns: [{ accessorKey: "visits", size: 70 }, { accessorKey: "status", minSize: 100 }, { accessorKey: "progress", maxSize: 80 }] }] },
    ],
    data: [{ firstName: "tanner", lastName: "linsley", age: 24, visits: 100, status: "single", progress: 50 }],
    steps: [
      { op: "resizeStart", header: "firstName", x: 150 },
      { op: "move", x: 180.5 },
      { op: "move", x: 90 },
      { op: "end", x: 100 },
      { op: "resizeStart", header: "1_Info_2_age_age", x: 400 },
      { op: "move", x: 520 },
      { op: "end", x: 530 },
      { op: "resizeStart", header: "2_More Info_visits", x: 600 },
      { op: "end", x: 640 },
      { op: "resizeStart", header: "status", x: 600 },
      { op: "end", x: -5000 },
      { op: "resizeStart", header: "1_Name_2_firstName_firstName", touches: [300.5] },
      { op: "move", touch: true, x: 340.25 },
      { op: "end", touch: true },
      { op: "resizeStart", header: "progress", touches: [10, 20] },
      { op: "resetSize", column: "age" },
      { op: "resetColumnSizing", defaultState: true },
    ],
  },
  {
    name: "resize-on-change-rtl",
    columns: [{ accessorKey: "a" }, { accessorKey: "b", size: 0 }, { accessorKey: "c", size: 33.333 }],
    options: { columnResizeMode: "onChange", columnResizeDirection: "rtl" },
    data: [{ a: 1, b: 2, c: 3 }],
    steps: [
      { op: "resizeStart", header: "a", x: 200 },
      { op: "move", x: 150.3 },
      { op: "move", x: 260 },
      { op: "end", x: 255 },
      { op: "resizeStart", header: "b", x: 10 },
      { op: "move", x: 0 },
      { op: "end", x: -20 },
      { op: "resizeStart", header: "c", touches: [99.5] },
      { op: "move", touch: true, x: 50 },
      { op: "end", cancel: true },
    ],
  },
];

// ------------------------------------------------------------------ main

const cases = [];
for (const hand of HAND) cases.push({ ...hand, data: encode(decode(hand.data)) });
for (let seed = 1; seed <= 160; seed++) cases.push(randomCase(seed, { rows: 1 + (seed % 23), steps: 12 }));
// Long inputs reach TimSort's run merging and galloping, where an
// inconsistent comparator (NaN, mixed types) decides the order.
for (const [seed, rows] of [[1001, 70], [1002, 200], [1003, 700], [1004, 1500]]) {
  cases.push(randomCase(seed, { rows, steps: 6, sortOnly: true }));
}

const lines = [];
for (const spec of cases) {
  lines.push(`    ${JSON.stringify({ ...spec, snapshots: run(spec) })}`);
}
const comment =
  "Generated by tools/table-oracle/generate.mjs; do not edit. Snapshot 0 is after construction, snapshot n after step n.";
writeFileSync(
  output,
  [
    "{",
    `  "comment": ${JSON.stringify(comment)},`,
    `  "packages": ${JSON.stringify(`@tanstack/table-core ${PACKAGE.version}, @tanstack/store ${STORE}, Node ${process.versions.node} (V8 ${process.versions.v8})`)},`,
    '  "cases": [',
    lines.join(",\n"),
    "  ]",
    "}",
    "",
  ].join("\n"),
);
console.log(`${cases.length} cases, ${cases.reduce((n, c) => n + c.steps.length + 1, 0)} snapshots`);
