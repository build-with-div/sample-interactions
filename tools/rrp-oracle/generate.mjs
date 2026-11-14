// Generates tests/fixtures/rrp-oracle.json: the outputs of
// react-resizable-panels' own layout and cursor functions, at the pinned
// version, over a seeded set of inputs. The Rust port in src/panels must
// reproduce every output exactly (tests/panels_oracle.rs).
//
// Usage (Node 22.18 or newer, which strips TypeScript types):
//
//   git clone --depth 1 --branch 4.22.1 \
//     https://github.com/bvaughn/react-resizable-panels out/rrp-4.22.1
//   node tools/rrp-oracle/generate.mjs out/rrp-4.22.1 tests/fixtures/rrp-oracle.json
//
// The script copies the pure functions it calls into out/rrp-oracle-src,
// adding the `.ts` extensions Node's resolver needs; nothing else changes.

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

const PACKAGE = "react-resizable-panels";
const VERSION = "4.22.1";
const COMMIT = "71c725362545da96b5d72eb6f7c3ca6832bb3b09";

const [checkout, output] = process.argv.slice(2);
if (!checkout || !output) {
  console.error("usage: node generate.mjs <checkout> <output.json>");
  process.exit(2);
}

const head = execFileSync("git", ["-C", checkout, "rev-parse", "HEAD"], {
  encoding: "utf8"
}).trim();
if (head !== COMMIT) {
  console.error(`expected ${PACKAGE} ${VERSION} at ${COMMIT}, found ${head}`);
  process.exit(1);
}

const FILES = [
  "constants.ts",
  "shared/utils/assert.ts",
  "shared/utils/isArrayEqual.ts",
  "global/utils/adjustLayoutByDelta.ts",
  "global/utils/calculateDefaultLayout.ts",
  "global/utils/calculateSeparatorAriaValues.ts",
  "global/utils/compareLayoutNumbers.ts",
  "global/utils/formatLayoutNumber.ts",
  "global/utils/layoutNumbersEqual.ts",
  "global/utils/layoutsEqual.ts",
  "global/utils/validateAxisLayout.ts",
  "global/utils/validateItemSize.ts",
  "global/cursor/getCursorStyle.ts",
  "global/cursor/supportsAdvancedCursorStyles.ts"
];

const staging = path.resolve("out/rrp-oracle-src");
fs.rmSync(staging, { recursive: true, force: true });
for (const file of FILES) {
  const source = fs.readFileSync(path.join(checkout, "lib", file), "utf8");
  // Relative value imports gain ".ts"; type-only imports are erased by Node.
  const rewritten = source.replace(
    /from "(\.{1,2}\/[^"]+)"/g,
    (match, specifier) =>
      match.startsWith("from") && !specifier.endsWith(".ts")
        ? `from "${specifier}.ts"`
        : match
  );
  const target = path.join(staging, file);
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.writeFileSync(target, rewritten);
}

const load = (file) => import(pathToFileURL(path.join(staging, file)).href);
const { adjustLayoutByDelta } = await load("global/utils/adjustLayoutByDelta.ts");
const { validateAxisLayout } = await load("global/utils/validateAxisLayout.ts");
const { validateItemSize } = await load("global/utils/validateItemSize.ts");
const { calculateDefaultLayout } = await load("global/utils/calculateDefaultLayout.ts");
const { calculateSeparatorAriaValues } = await load(
  "global/utils/calculateSeparatorAriaValues.ts"
);
const { formatLayoutNumber } = await load("global/utils/formatLayoutNumber.ts");
const { getCursorStyle } = await load("global/cursor/getCursorStyle.ts");
const { overrideSupportsAdvancedCursorStylesForTesting } = await load(
  "global/cursor/supportsAdvancedCursorStyles.ts"
);

// mulberry32: a small deterministic PRNG, so the fixture is reproducible.
let seed = 0x5eed1234;
function random() {
  seed |= 0;
  seed = (seed + 0x6d2b79f5) | 0;
  let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
}
const pick = (values) => values[Math.floor(random() * values.length)];
const between = (low, high) => low + random() * (high - low);
// Mix round numbers (where boundaries and ties live) with arbitrary ones.
const amount = (low, high) =>
  random() < 0.5 ? Math.round(between(low, high)) : between(low, high);

function randomConstraints(count) {
  const constraints = [];
  for (let index = 0; index < count; index++) {
    const minSize = random() < 0.4 ? 0 : amount(0, 40);
    const maxSize = random() < 0.5 ? 100 : amount(minSize, 100);
    const collapsible = random() < 0.35;
    const collapsedSize = collapsible && random() < 0.5 ? amount(0, minSize) : 0;
    constraints.push({
      collapsedSize,
      collapsedThreshold:
        collapsible && random() < 0.3 ? amount(0, minSize - collapsedSize + 2) : undefined,
      collapsible,
      defaultSize: random() < 0.5 ? amount(minSize, maxSize) : undefined,
      disabled: random() < 0.1 ? true : undefined,
      maxSize,
      minSize,
      itemId: String(index)
    });
  }
  return constraints;
}

function randomLayout(count) {
  const weights = Array.from({ length: count }, () => between(0.05, 1));
  const total = weights.reduce((a, b) => a + b, 0);
  const layout = {};
  weights.forEach((weight, index) => {
    layout[index] = random() < 0.3 ? Math.round((weight / total) * 100) : (weight / total) * 100;
  });
  return layout;
}

// The Rust side takes layouts as arrays in item order; JavaScript's object
// key order for "0".."n" is the same.
const toArray = (layout) => Object.values(layout);
const run = (operation) => {
  try {
    return { ok: operation() };
  } catch (error) {
    return { error: String(error.message) };
  }
};

const cases = {
  formatLayoutNumber: [],
  validateItemSize: [],
  adjustLayoutByDelta: [],
  validateAxisLayout: [],
  calculateDefaultLayout: [],
  separatorAriaValues: [],
  cursorStyle: []
};

for (let index = 0; index < 1000; index++) {
  const value =
    index < 400
      ? (index - 200) / 2000 + (index % 7) * 0.0000001
      : random() < 0.5
        ? between(-150, 150)
        : Math.round(between(-150000, 150000)) / 2000;
  cases.formatLayoutNumber.push({ value, result: formatLayoutNumber(value) });
}

for (let index = 0; index < 1000; index++) {
  const [constraints] = randomConstraints(1);
  const prevSize = amount(0, 100);
  const size = amount(-10, 110);
  const overrideDisabledItems = random() < 0.3;
  cases.validateItemSize.push({
    constraints,
    prevSize,
    size,
    overrideDisabledItems,
    result: validateItemSize({ overrideDisabledItems, itemConstraints: constraints, prevSize, size })
  });
}

// Each input is run with all three triggers.
const TRIGGERS = ["imperative-api", "keyboard", "mouse-or-touch"];
for (let index = 0; index < 2000; index++) {
  const count = pick([2, 2, 3, 3, 4, 5]);
  const constraints = randomConstraints(count);
  const initial = validateAxisLayout({ layout: randomLayout(count), itemConstraints: constraints });
  const prev =
    random() < 0.7
      ? initial
      : validateAxisLayout({ layout: randomLayout(count), itemConstraints: constraints });
  const first = Math.floor(random() * (count - 1));
  const delta = pick([
    () => amount(-110, 110),
    () => amount(-10, 10),
    () => pick([-100, -5, 5, 100])
  ])();
  const results = TRIGGERS.map((trigger) =>
    run(() =>
      toArray(
        adjustLayoutByDelta({
          delta,
          initialLayout: initial,
          itemConstraints: constraints,
          pivotIndices: [first, first + 1],
          prevLayout: prev,
          trigger
        })
      )
    )
  );
  cases.adjustLayoutByDelta.push({
    constraints,
    initialLayout: toArray(initial),
    prevLayout: prev === initial ? null : toArray(prev),
    pivotIndices: [first, first + 1],
    delta,
    triggers: TRIGGERS,
    results
  });
}

for (let index = 0; index < 1000; index++) {
  const count = pick([1, 2, 3, 4, 5]);
  const constraints = randomConstraints(count);
  const layout = {};
  for (let item = 0; item < count; item++) {
    layout[item] = amount(0, random() < 0.5 ? 60 : 120);
  }
  const result = run(() => toArray(validateAxisLayout({ layout, itemConstraints: constraints })));
  cases.validateAxisLayout.push({ constraints, layout: toArray(layout), ...result });
}

for (let index = 0; index < 300; index++) {
  const constraints = randomConstraints(pick([1, 2, 3, 4, 5]));
  cases.calculateDefaultLayout.push({
    constraints,
    result: toArray(calculateDefaultLayout(constraints))
  });
}

for (let index = 0; index < 1000; index++) {
  const count = pick([2, 3, 4]);
  const constraints = randomConstraints(count);
  const layout = validateAxisLayout({ layout: randomLayout(count), itemConstraints: constraints });
  const itemIndex = Math.floor(random() * (count - 1));
  const result = run(() => {
    const values = calculateSeparatorAriaValues({
      layout,
      itemConstraints: constraints,
      itemId: String(itemIndex),
      itemIndex
    });
    return [values.valueMin, values.valueMax, values.valueNow];
  });
  cases.separatorAriaValues.push({ constraints, layout: toArray(layout), itemIndex, ...result });
}

overrideSupportsAdvancedCursorStylesForTesting(true);
for (const state of ["inactive", "hover", "active"]) {
  for (let flags = 0; flags < 16; flags++) {
    for (const orientations of [[], ["horizontal"], ["vertical"], ["horizontal", "vertical"]]) {
      for (const disableCursor of [false, true]) {
        const axes = orientations.map((orientation) => ({
          orientation,
          mutableState: { disableCursor }
        }));
        cases.cursorStyle.push({
          state,
          cursorFlags: flags,
          orientations,
          disableCursor,
          result: getCursorStyle({ cursorFlags: flags, axes, state }) ?? null
        });
      }
    }
  }
}

// Constraints are written compactly, in this order:
// [collapsedSize, collapsedThreshold, collapsible, defaultSize, disabled, maxSize, minSize]
// with null for undefined and 0/1 for booleans.
const compact = (constraint) => [
  constraint.collapsedSize,
  constraint.collapsedThreshold ?? null,
  constraint.collapsible ? 1 : 0,
  constraint.defaultSize ?? null,
  constraint.disabled ? 1 : 0,
  constraint.maxSize,
  constraint.minSize
];
for (const list of Object.values(cases)) {
  for (const entry of list) {
    if (Array.isArray(entry.constraints)) {
      entry.constraints = entry.constraints.map(compact);
    } else if (entry.constraints) {
      entry.constraints = compact(entry.constraints);
    }
  }
}

const fixture = {
  source: {
    package: PACKAGE,
    version: VERSION,
    commit: COMMIT,
    node: process.version,
    generator: "tools/rrp-oracle/generate.mjs",
    supportsAdvancedCursorStyles: true
  },
  cases
};
fs.mkdirSync(path.dirname(output), { recursive: true });
fs.writeFileSync(output, JSON.stringify(fixture) + "\n");
const counts = Object.entries(cases).map(([name, list]) => `${name}=${list.length}`);
console.log(`wrote ${output}: ${counts.join(" ")}`);
