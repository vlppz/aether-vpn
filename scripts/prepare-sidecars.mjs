#!/usr/bin/env node
// Builds `aether-helper` and places it where Tauri expects external binaries
// (`src-tauri/binaries/<name>-<target-triple>[.exe]`). Also checks that the
// Xray sidecar has been fetched. Cross-platform; run from anywhere.
import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, chmodSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const release = process.argv.includes("--release");
const targetArg = process.argv.find((a) => a.startsWith("--target="))?.slice(9);
const triple =
  targetArg ??
  process.env.TAURI_ENV_TARGET_TRIPLE ??
  execFileSync("rustc", ["-vV"], { encoding: "utf8" }).match(/^host: (.+)$/m)[1];
const exe = triple.includes("windows") ? ".exe" : "";

// Cross-compiling to Windows MSVC from macOS/Linux needs cargo-xwin for the
// MSVC CRT/SDK and linker (same runner `tauri build --runner cargo-xwin` uses).
const crossWindows = exe && process.platform !== "win32";
const args = [...(crossWindows ? ["xwin"] : []), "build", "-p", "aether-helper", "--target", triple];
if (release) args.push("--release");
execFileSync("cargo", args, { cwd: root, stdio: "inherit" });

const built = join(root, "target", triple, release ? "release" : "debug", `aether-helper${exe}`);
const dest = join(root, "src-tauri", "binaries", `aether-helper-${triple}${exe}`);
copyFileSync(built, dest);
if (!exe) chmodSync(dest, 0o755);
console.log(`✓ aether-helper → ${dest}`);

const xray = join(root, "src-tauri", "binaries", `xray-${triple}${exe}`);
if (!existsSync(xray)) {
  console.error(`✗ missing ${xray}\n  run: scripts/fetch-core.sh ${triple}   (or scripts/fetch-core.ps1 on Windows)`);
  process.exit(1);
}
