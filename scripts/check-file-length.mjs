#!/usr/bin/env node
// 代码门禁：单个源码文件不得超过 MAX_LINES 行,超出则拆分为模块/文件。
// 这是 CLAUDE.md 中记录的事实标准,前后端源码一并扫描,详见该文件“代码门禁”一节。
// 用法：node scripts/check-file-length.mjs（已接入 `pnpm run check`）。

import { readdirSync, readFileSync, statSync } from "node:fs";
import { extname, join } from "node:path";

const MAX_LINES = 400; // 门禁阈值：任意一个源码文件的行数上限

// 参与扫描的目录：前端源码 + Rust 后端源码。
const SCAN_ROOTS = ["src", "src-tauri/src"];
// 只统计这些扩展名，避免样式表、生成产物等非结构化代码被计入。
const SCAN_EXTENSIONS = new Set([".ts", ".tsx", ".rs"]);
// 跳过的目录：依赖、构建产物、Tauri 生成的绑定代码，都不是手写源码。
const IGNORED_DIR_NAMES = new Set(["node_modules", "target", "dist", "gen"]);

// 递归收集某个根目录下所有匹配扩展名的文件路径。
function collectFiles(root) {
  const results = [];
  const walk = (dir) => {
    let entries;
    try {
      entries = readdirSync(dir, { withFileTypes: true });
    } catch {
      return; // 目录不存在（例如尚未 `cargo build` 生成过 target）时直接跳过
    }
    for (const entry of entries) {
      if (entry.isDirectory()) {
        if (IGNORED_DIR_NAMES.has(entry.name)) continue; // 跳过依赖/产物目录
        walk(join(dir, entry.name));
        continue;
      }
      if (SCAN_EXTENSIONS.has(extname(entry.name))) {
        results.push(join(dir, entry.name));
      }
    }
  };
  walk(root);
  return results;
}

// 统计一个文件的行数（按换行符个数计算，空文件视为 0 行）。
function countLines(path) {
  const content = readFileSync(path, "utf8");
  if (content.length === 0) return 0;
  return content.split("\n").length;
}

const violations = SCAN_ROOTS.flatMap((root) =>
  collectFiles(root)
    .map((path) => ({ path, lines: countLines(path) }))
    .filter(({ lines }) => lines > MAX_LINES),
);

if (violations.length > 0) {
  console.error(`\n代码门禁未通过：以下文件超过单文件 ${MAX_LINES} 行的上限，请拆分为更小的模块/文件：\n`);
  for (const { path, lines } of violations.sort((a, b) => b.lines - a.lines)) {
    console.error(`  ${path}  (${lines} 行)`);
  }
  console.error("\n拆分原则见 CLAUDE.md 中的“代码门禁”一节。\n");
  process.exit(1);
}

const scannedCount = SCAN_ROOTS.flatMap(collectFiles).length;
console.log(`代码门禁通过：已扫描 ${scannedCount} 个源码文件，均未超过 ${MAX_LINES} 行。`);
