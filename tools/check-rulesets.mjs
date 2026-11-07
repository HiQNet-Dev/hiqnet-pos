#!/usr/bin/env node
// Comprobación ligera de la coherencia documentada de los rulesets propuestos.
// No aplica reglas: solo valida JSON y las invariantes del flujo actual.

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const readJson = (file) => JSON.parse(readFileSync(path.join(root, file), "utf8"));

let errors = [];
function fail(msg) { errors.push(msg); }

const main = readJson("docs/07-maintenance/ruleset-main.json");
const tags = readJson("docs/07-maintenance/ruleset-tags.json");

if (main.target !== "branch" || !main.conditions?.ref_name?.include?.includes("refs/heads/main")) {
  fail("ruleset-main.json: target/condición de main inválida");
}
const mainTypes = (main.rules || []).map((r) => r.type);
for (const required of ["deletion", "non_fast_forward", "pull_request", "required_status_checks"]) {
  if (!mainTypes.includes(required)) fail(`ruleset-main.json: falta regla ${required}`);
}
if (mainTypes.includes("required_linear_history")) {
  fail("ruleset-main.json: no debe incluir required_linear_history (merge commits)");
}
const pr = (main.rules || []).find((r) => r.type === "pull_request")?.parameters || {};
if (pr.required_approving_review_count !== 0) {
  fail("ruleset-main.json: required_approving_review_count debe ser 0 para mantenedor único");
}
if (!Array.isArray(pr.allowed_merge_methods) || !pr.allowed_merge_methods.includes("merge")) {
  fail("ruleset-main.json: allowed_merge_methods debe permitir merge commits");
}
const checks = (main.rules || [])
  .find((r) => r.type === "required_status_checks")
  ?.parameters?.required_status_checks?.map((c) => c.context) || [];
const expectedChecks = [
  "Rust (fmt, clippy, tests)",
  "Frontend (lint, tipos, pruebas, build)",
  "Tauri Windows (build de verificación)",
  "Tauri Linux (compilación real)",
  "Archivos prohibidos y auditoría",
];
if (JSON.stringify(checks) !== JSON.stringify(expectedChecks)) {
  fail("ruleset-main.json: checks obligatorios incoherentes");
}

if (tags.target !== "tag" || !tags.conditions?.ref_name?.include?.includes("refs/tags/pos-v*")) {
  fail("ruleset-tags.json: target/condición de tags inválida");
}
const tagTypes = (tags.rules || []).map((r) => r.type);
if (!tagTypes.includes("deletion") || !tagTypes.includes("update")) {
  fail("ruleset-tags.json: faltan deletion/update");
}
if (tagTypes.includes("creation")) {
  fail("ruleset-tags.json: no debe bloquear creation sin mecanismo de excepción");
}

if (errors.length) {
  console.error("Inconsistencia en rulesets propuestos:");
  for (const e of errors) console.error(`- ${e}`);
  process.exit(1);
}
console.log("OK: rulesets propuestos coherentes con el flujo actual.");
