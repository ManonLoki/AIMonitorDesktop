#!/usr/bin/env node

import { createHash } from "node:crypto";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { platform } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import process from "node:process";
import { fileURLToPath } from "node:url";

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const tauriRoot = join(projectRoot, "src-tauri");
const publishDir = join(projectRoot, "publish");
const projectCargoTargetDir = join(tauriRoot, "target");
const packageJson = JSON.parse(readFileSync(join(projectRoot, "package.json"), "utf8"));
const version = packageJson.version;
const requestedPlatform = process.argv[2] ?? "all";
const supportedPlatforms = new Set(["mac", "win", "all"]);

if (!supportedPlatforms.has(requestedPlatform)) {
  throw new Error("用法：node scripts/build-release.mjs [mac|win|all]");
}

function run(command, args, env = process.env) {
  console.log(`\n> ${command} ${args.join(" ")}`);
  const result = spawnSync(command, args, {
    cwd: projectRoot,
    env,
    stdio: "inherit",
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(`${command} 退出，状态码 ${result.status}`);
  }
}

function commandExists(command, env = process.env) {
  const probe = spawnSync(command, ["--version"], {
    cwd: projectRoot,
    env,
    stdio: "ignore",
  });
  return !probe.error;
}

function filesBelow(directory, extension) {
  if (!existsSync(directory)) return [];
  const files = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const entryPath = join(directory, entry.name);
    if (entry.isDirectory()) files.push(...filesBelow(entryPath, extension));
    if (entry.isFile() && entry.name.toLowerCase().endsWith(extension)) files.push(entryPath);
  }
  return files;
}

function newestArtifact(directory, extension) {
  const candidates = filesBelow(directory, extension)
    .sort((left, right) => statSync(right).mtimeMs - statSync(left).mtimeMs);
  if (candidates.length === 0) {
    throw new Error(`没有在 ${directory} 找到 ${extension} 构建产物`);
  }
  return candidates[0];
}

function buildMac() {
  if (platform() !== "darwin") {
    throw new Error("macOS DMG 必须在 macOS 主机上构建");
  }
  const target = process.env.AIMONITOR_MAC_TARGET ?? "universal-apple-darwin";
  if (target === "universal-apple-darwin") {
    run("rustup", ["target", "add", "aarch64-apple-darwin", "x86_64-apple-darwin"]);
  } else {
    run("rustup", ["target", "add", target]);
  }
  const env = {
    ...process.env,
    CARGO_TARGET_DIR: projectCargoTargetDir,
  };
  run("pnpm", ["tauri", "build", "--target", target, "--ci"], env);
  const source = newestArtifact(
    join(tauriRoot, "target", target, "release", "bundle", "dmg"),
    ".dmg",
  );
  run("codesign", ["--verify", "--strict", "--verbose=2", source], env);
  const architecture = target === "universal-apple-darwin"
    ? "universal"
    : target.startsWith("aarch64")
      ? "arm64"
      : "x64";
  return {
    source,
    filename: `AIMonitorDesktop-macOS-${architecture}-v${version}.dmg`,
  };
}

function buildWindows() {
  if (platform() === "win32") {
    throw new Error("build:win 使用 xwin 交叉编译，请在 macOS 或 Linux 主机运行");
  }
  const target = "x86_64-pc-windows-msvc";
  const llvmPaths = [
    "/opt/homebrew/opt/llvm/bin",
    "/usr/local/opt/llvm/bin",
    "/usr/lib/llvm/bin",
  ].filter(existsSync);
  const env = {
    ...process.env,
    CARGO_TARGET_DIR: projectCargoTargetDir,
    PATH: [...llvmPaths, process.env.PATH].filter(Boolean).join(":"),
    XWIN_CACHE_DIR: process.env.XWIN_CACHE_DIR ?? join(projectRoot, ".xwin-cache"),
  };
  for (const command of ["cargo-xwin", "makensis", "llvm-rc"]) {
    if (!commandExists(command, env)) {
      throw new Error(`Windows 交叉构建缺少命令：${command}`);
    }
  }
  run("rustup", ["target", "add", target], env);
  run(
    "pnpm",
    ["tauri", "build", "--runner", "cargo-xwin", "--target", target, "--ci", "--no-sign"],
    env,
  );
  const source = newestArtifact(
    join(tauriRoot, "target", target, "release", "bundle", "nsis"),
    ".exe",
  );
  return {
    source,
    filename: `AIMonitorDesktop-Windows-x64-v${version}-setup.exe`,
  };
}

const artifacts = [];
if (requestedPlatform === "mac" || requestedPlatform === "all") artifacts.push(buildMac());
if (requestedPlatform === "win" || requestedPlatform === "all") artifacts.push(buildWindows());

rmSync(publishDir, { recursive: true, force: true });
mkdirSync(publishDir, { recursive: true });

const checksums = [];
for (const artifact of artifacts) {
  const destination = join(publishDir, artifact.filename);
  copyFileSync(artifact.source, destination);
  const checksum = createHash("sha256").update(readFileSync(destination)).digest("hex");
  checksums.push(`${checksum}  ${basename(destination)}`);
  console.log(`已发布：${destination}`);
}
writeFileSync(
  join(publishDir, "AIMonitorDesktop-SHA256SUMS.txt"),
  `${checksums.join("\n")}\n`,
);
console.log(`\npublish 已清理并写入 ${artifacts.length} 个 AIMonitorDesktop 安装包。`);
