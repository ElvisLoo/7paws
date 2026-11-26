// One source of truth for the built-in cat roster.
//
// Four places describe the same 13 cats and can drift silently:
//   src/pet/coats.ts        UI swatches + debug coat picker (TypeScript)
//   public/vendor/7paws.js  the renderer that actually draws the pet
//   src-tauri/src/config.rs COATS id list used to validate settings.json
//   src-tauri/src/mcp.rs    BUILTIN id/name pairs advertised over MCP
//
// Exit code 1 when a hard invariant breaks (missing/renamed coat, wrong
// display name, different fur colour, pattern mismatch); softer colour deltas
// are reported but do not fail CI.
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (path) => readFileSync(join(root, path), "utf8");

const tsSource = read("src/pet/coats.ts");
const vendorSource = read("public/vendor/7paws.js");
const rustConfig = read("src-tauri/src/config.rs");
const mcpSource = read("src-tauri/src/mcp.rs");

function parseTsCoats(source) {
  const body = source.match(/export const COATS: Coat\[\] = \[([\s\S]*?)\n\];/);
  if (!body) throw new Error("could not locate COATS array in src/pet/coats.ts");
  const coats = new Map();
  for (const block of body[1].match(/\{[^{}]*\}/g) ?? []) {
    const fields = parseFields(block);
    if (fields.id) coats.set(fields.id, fields);
  }
  return coats;
}

function parseVendorCoats(source) {
  const body = source.match(/const COATS = \[([\s\S]*?)\n\];/);
  if (!body) throw new Error("could not locate COATS array in public/vendor/7paws.js");
  const coats = new Map();
  for (const block of body[1].match(/\{[^{}]*\}/g) ?? []) {
    const fields = parseFields(block);
    if (fields.id) coats.set(fields.id, fields);
  }
  return coats;
}

function parseFields(block) {
  const fields = {};
  for (const match of block.matchAll(/(?:^|[{,\s])([A-Za-z_][\w]*):\s*(?:"([^"]*)"|'([^']*)'|(-?\d+(?:\.\d+)?))/g)) {
    const key = match[1];
    const value = match[2] ?? match[3] ?? match[4];
    if (value !== undefined) fields[key] = value;
  }
  return fields;
}

function parseRustIds(source) {
  const body = source.match(/pub\(crate\) const COATS: &\[&str\] = &\[([\s\S]*?)\];/);
  if (!body) throw new Error("could not locate COATS in src-tauri/src/config.rs");
  return new Set(body[1].matchAll(/"([^"]+)"/g).map((m) => m[1]));
}

function parseBuiltin(source) {
  const body = source.match(/const BUILTIN: &\[\(&str, &str\)\] = &\[([\s\S]*?)\];/);
  if (!body) throw new Error("could not locate BUILTIN in src-tauri/src/mcp.rs");
  const map = new Map();
  for (const match of body[1].matchAll(/\("([^"]+)",\s*"([^"]+)"\)/g)) {
    map.set(match[1], match[2]);
  }
  return map;
}

function parseRustPatterns(source) {
  const body = source.match(/const PATTERNS: \[&str; \d+\] = \[([^\]]*)\]/);
  if (!body) throw new Error("could not locate PATTERNS in src-tauri/src/cats.rs");
  return new Set(body[1].matchAll(/"([^"]+)"/g).map((m) => m[1]));
}

const ts = parseTsCoats(tsSource);
const vendor = parseVendorCoats(vendorSource);
const rustIds = parseRustIds(rustConfig);
const builtin = parseBuiltin(mcpSource);
const rustPatterns = parseRustPatterns(read("src-tauri/src/cats.rs"));

const problems = [];
const warnings = [];

const unionIds = [...new Set([...ts.keys(), ...vendor.keys(), ...rustIds, ...builtin.keys()])].sort();
for (const id of unionIds) {
  if (!ts.has(id)) problems.push(`src/pet/coats.ts 缺少猫 ${id}`);
  if (!vendor.has(id)) problems.push(`public/vendor/7paws.js 缺少猫 ${id}`);
  if (!rustIds.has(id)) problems.push(`src-tauri/src/config.rs 的 COATS 缺少猫 ${id}`);
  if (!builtin.has(id)) problems.push(`src-tauri/src/mcp.rs 的 BUILTIN 缺少猫 ${id}`);
}

for (const [id, tsCoat] of ts) {
  const vendorCoat = vendor.get(id);
  if (!vendorCoat) continue;
  const expectedName = builtin.get(id);
  if (tsCoat.name !== vendorCoat.name) {
    problems.push(`${id} 名称不一致：TS「${tsCoat.name}」 vs 渲染器「${vendorCoat.name}」`);
  }
  if (expectedName && tsCoat.name !== expectedName) {
    problems.push(`${id} 名称不一致：TS「${tsCoat.name}」 vs MCP「${expectedName}」`);
  }
  // Fur is the dominant coat colour and what the swatch previews.
  if (tsCoat.fur?.toLowerCase() !== vendorCoat.fur?.toLowerCase()) {
    problems.push(`${id} 主色不一致：TS ${tsCoat.fur} vs 渲染器 ${vendorCoat.fur}`);
  }
  if ((tsCoat.pattern ?? "solid") !== (vendorCoat.pattern ?? "solid")) {
    problems.push(`${id} 花纹不一致：TS ${tsCoat.pattern ?? "solid"} vs 渲染器 ${vendorCoat.pattern ?? "solid"}`);
  }
  for (const key of ["eye", "inner", "whisker", "line", "stripe", "tail", "patchA", "patchB"]) {
    const a = tsCoat[key];
    const b = vendorCoat[key];
    if (a === undefined || b === undefined) continue;
    if (String(a).toLowerCase() !== String(b).toLowerCase()) {
      warnings.push(`${id}.${key}：TS ${a} vs 渲染器 ${b}`);
    }
  }
}

const tsPatterns = new Set([...ts.values()].map((c) => c.pattern ?? "solid"));
const vendorPatterns = new Set([...vendor.values()].map((c) => c.pattern ?? "solid"));
for (const p of tsPatterns) {
  if (!rustPatterns.has(p)) {
    warnings.push(`自定义猫花纹「${p}」不在 Rust PATTERNS 中，后端会回退成默认值（内置渲染不受影响）`);
  }
}
for (const p of vendorPatterns) {
  if (!tsPatterns.has(p)) warnings.push(`渲染器花纹「${p}」未出现在 src/pet/coats.ts`);
}

console.log(`cats: TS=${ts.size} vendor=${vendor.size} rust=${rustIds.size} mcp=${builtin.size}`);
if (warnings.length) {
  console.warn(`\n提示（${warnings.length}）:`);
  for (const w of warnings) console.warn(`  - ${w}`);
}
if (problems.length) {
  console.error(`\n失败（${problems.length}）:`);
  for (const p of problems) console.error(`  - ${p}`);
  process.exit(1);
}
console.log("OK：猫名录与渲染器一致。");
