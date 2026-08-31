import { readdirSync, readFileSync } from "node:fs";
import { extname, join, relative } from "node:path";

const root = process.cwd();
const includedExtensions = new Set([".md", ".json", ".mjs", ".rs", ".svelte", ".toml", ".ts"]);
const ignoredDirectories = new Set([".git", ".repochan", "dist", "node_modules", "target"]);

function repositoryPath(path) {
  return relative(root, path).split("\\").join("/");
}

function sourceFiles(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    if (ignoredDirectories.has(entry.name)) return [];
    const path = join(directory, entry.name);
    if (entry.isDirectory()) return sourceFiles(path);
    if (!includedExtensions.has(extname(entry.name))) return [];
    return [path];
  });
}

const legacyTerms = [
  /\bProjectInput\b/,
  /\bproject_root\b/,
  /\bagent_sessions\b/,
  /\bWakeup(?:Adapter|Dispatcher|Payload|Result)\b/,
  /\brambledesk-adapters\b/,
  /\bRAMBLEDESK_MCP_PORT\b/,
  /\badapter_id\b/,
];

const violations = [];
for (const path of sourceFiles(root)) {
  const displayPath = repositoryPath(path);
  if (displayPath === "docs/TERMINOLOGY.md" || displayPath === "scripts/check-terminology.mjs") continue;
  const lines = readFileSync(path, "utf8").split(/\r?\n/);
  lines.forEach((line, index) => {
    for (const pattern of legacyTerms) {
      if (pattern.test(line)) violations.push(`${displayPath}:${index + 1}: ${pattern}`);
    }
  });
}

const coreFiles = sourceFiles(join(root, "crates/rambledesk-core"));
const forbiddenCoreTerms = [/\baxum\b/, /\brmcp\b/, /\bserde_json\b/, /\btauri\b/, /rambledesk_(?:hosts|local_server|mcp|storage)/];
for (const path of coreFiles) {
  const displayPath = repositoryPath(path);
  const lines = readFileSync(path, "utf8").split(/\r?\n/);
  lines.forEach((line, index) => {
    for (const pattern of forbiddenCoreTerms) {
      if (pattern.test(line)) violations.push(`${displayPath}:${index + 1}: core boundary ${pattern}`);
    }
  });
}

const dependencyContracts = new Map([
  ["crates/rambledesk-core/Cargo.toml", []],
  ["crates/rambledesk-storage/Cargo.toml", ["rambledesk-core"]],
  ["crates/rambledesk-mcp/Cargo.toml", ["rambledesk-core", "rambledesk-hosts"]],
  ["crates/rambledesk-local-server/Cargo.toml", ["rambledesk-core", "rambledesk-mcp"]],
  ["crates/rambledesk-hosts/Cargo.toml", ["rambledesk-core"]],
  ["crates/rambledesk-speech/Cargo.toml", []],
  ["crates/rambledesk-cli/Cargo.toml", ["rambledesk-core", "rambledesk-local-server", "rambledesk-storage"]],
  [
    "apps/desktop/src-tauri/Cargo.toml",
    [
      "rambledesk-acp-client",
      "rambledesk-core",
      "rambledesk-hosts",
      "rambledesk-local-server",
      "rambledesk-mcp",
      "rambledesk-speech",
      "rambledesk-storage",
    ],
  ],
]);
for (const [manifest, expected] of dependencyContracts) {
  const contents = readFileSync(join(root, manifest), "utf8");
  const dependencies = contents.split(/\[dependencies\]\r?\n/, 2)[1]?.split(/\r?\n\[/, 1)[0] ?? "";
  const actual = [...dependencies.matchAll(/^(rambledesk-[a-z-]+)\.workspace\s*=\s*true$/gm)]
    .map((match) => match[1])
    .sort();
  if (actual.join("\0") !== [...expected].sort().join("\0")) {
    violations.push(`${manifest}: workspace dependency boundary expected [${expected.join(", ")}], found [${actual.join(", ")}]`);
  }
}

if (violations.length > 0) {
  console.error("Terminology or package-boundary drift detected:\n" + violations.join("\n"));
  process.exit(1);
}

console.log("Terminology and core-boundary checks passed.");
