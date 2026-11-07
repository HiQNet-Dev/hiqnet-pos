#!/usr/bin/env node
// Verifica que el entorno local coincide con las versiones fijadas del
// repositorio. No instala nada: falla con un diagnóstico accionable.
//
// Uso: node tools/check-toolchain.mjs

import { readFileSync, readdirSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const read = (relative) => readFileSync(path.join(root, relative), "utf8");

function run(command, args = []) {
  if (process.platform === "win32") {
    // Node rechaza ejecutar .cmd directamente sin shell; cmd.exe conserva los
    // argumentos sin interpolación.
    return execFileSync("cmd.exe", ["/c", command, ...args], {
      encoding: "utf8",
    }).trim();
  }
  return execFileSync(command, args, { encoding: "utf8" }).trim();
}

function firstLine(value) {
  return value.split(/\r?\n/, 1)[0];
}

const expectedRust = /^\s*channel\s*=\s*"([^"]+)"/m.exec(
  read("rust-toolchain.toml"),
)?.[1];
const expectedNode = read(".node-version").trim();
const frontendPackage = JSON.parse(read("frontend/package.json"));
const expectedPnpm = frontendPackage.packageManager?.split("@").at(-1);
const expectedCli = /@tauri-apps\/cli@([0-9][^ "'`]*)/.exec(read("justfile"))?.[1];

const problems = [];
const notes = [];

if (!expectedRust) problems.push("rust-toolchain.toml no declara channel.");
if (!expectedNode) problems.push(".node-version está vacío.");
if (!expectedPnpm) problems.push("frontend/package.json no declara packageManager.");
if (!expectedCli) problems.push("justfile no fija @tauri-apps/cli.");

try {
  const rustc = firstLine(run("rustc", ["--version"])).split(" ")[1];
  if (expectedRust && rustc !== expectedRust) {
    problems.push(`rustc ${rustc} != ${expectedRust} (rust-toolchain.toml).`);
  } else {
    notes.push(`rustc ${rustc}`);
  }
} catch {
  problems.push("rustc no está disponible en PATH.");
}

try {
  const node = firstLine(run("node", ["--version"]));
  if (expectedNode && node !== `v${expectedNode}`) {
    problems.push(`node ${node} != v${expectedNode} (.node-version).`);
  } else {
    notes.push(`node ${node}`);
  }
} catch {
  problems.push("node no está disponible en PATH.");
}

try {
  const pnpm = firstLine(run("pnpm", ["--version"]));
  if (expectedPnpm && pnpm !== expectedPnpm) {
    problems.push(`pnpm ${pnpm} != ${expectedPnpm} (packageManager).`);
  } else {
    notes.push(`pnpm ${pnpm}`);
  }
} catch {
  problems.push("pnpm no está disponible en PATH.");
}

// El CLI de Tauri no debe quedar flotando en `@2` ni divergir entre scripts.
if (expectedCli) {
  const workflows = path.join(root, ".github", "workflows");
  const files = [
    "justfile",
    "README.md",
    "CONTRIBUTING.md",
    "README-TESTING.md",
    "docs/02-development/README.md",
    "docs/02-development/windows.md",
    "docs/02-development/android.md",
    "docs/05-operations/windows-installation.md",
    "docs/06-release/versioning.md",
    "tools/prepilot-sandbox/Prepare-SandboxArtifacts.ps1",
    "tools/windows-native-e2e/README.md",
    "tools/windows-native-e2e/run-e2e.ps1",
    ...readdirSync(workflows)
      .filter((name) => name.endsWith(".yml"))
      .map((name) => `.github/workflows/${name}`),
  ];
  for (const file of files) {
    let content;
    try {
      content = read(file);
    } catch {
      continue;
    }
    for (const match of content.matchAll(/@tauri-apps\/cli@([^ "'`\n]+)/g)) {
      if (!/^[0-9]+\.[0-9]+\.[0-9]+/.test(match[1])) {
        problems.push(`${file}: CLI de Tauri flotante (${match[1]}).`);
      } else if (match[1] !== expectedCli) {
        problems.push(`${file}: CLI ${match[1]} != ${expectedCli} (justfile).`);
      }
    }
  }
  notes.push(`tauri-cli ${expectedCli}`);
}

if (problems.length > 0) {
  console.error("Entorno no coincide con las versiones fijadas:");
  for (const problem of problems) console.error(`- ${problem}`);
  console.error("Corrige el toolchain o actualiza la fuente de verdad correspondiente.");
  process.exit(1);
}

console.log(`OK: ${notes.join(" · ")}`);
