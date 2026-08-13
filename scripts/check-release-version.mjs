import { readFileSync } from "node:fs";

const readJson = (path) => JSON.parse(readFileSync(path, "utf8"));

const rootVersion = readJson("package.json").version;
const versions = new Map([
  ["package.json", rootVersion],
  ["apps/desktop/package.json", readJson("apps/desktop/package.json").version],
  ["packages/pi-rambledesk/package.json", readJson("packages/pi-rambledesk/package.json").version],
  ["packages/dsh-rambledesk/package.json", readJson("packages/dsh-rambledesk/package.json").version],
  ["apps/desktop/src-tauri/tauri.conf.json", readJson("apps/desktop/src-tauri/tauri.conf.json").version],
]);

const cargoWorkspace = readFileSync("Cargo.toml", "utf8").match(
  /\[workspace\.package\][\s\S]*?^version\s*=\s*"([^"]+)"/m,
);
if (!cargoWorkspace) {
  throw new Error("Could not read [workspace.package].version from Cargo.toml");
}
versions.set("Cargo.toml [workspace.package]", cargoWorkspace[1]);

const cargoLock = readFileSync("Cargo.lock", "utf8");
for (const packageName of [
  "rambledesk-cli",
  "rambledesk-core",
  "rambledesk-desktop",
  "rambledesk-hosts",
  "rambledesk-local-server",
  "rambledesk-mcp",
  "rambledesk-speech",
  "rambledesk-storage",
]) {
  const packageBlock = cargoLock.match(
    new RegExp(`\\[\\[package\\]\\]\\nname = "${packageName}"\\nversion = "([^"]+)"`),
  );
  if (!packageBlock) throw new Error(`Could not read ${packageName} from Cargo.lock`);
  versions.set(`Cargo.lock ${packageName}`, packageBlock[1]);
}

const mismatches = [...versions].filter(([, version]) => version !== rootVersion);
if (mismatches.length > 0) {
  throw new Error(
    `Release versions do not match ${rootVersion}: ${mismatches
      .map(([path, version]) => `${path}=${version}`)
      .join(", ")}`,
  );
}

const tag = process.env.GITHUB_REF_TYPE === "tag" ? process.env.GITHUB_REF_NAME : process.argv[2];
if (tag && tag !== `v${rootVersion}`) {
  throw new Error(`Release tag ${tag} does not match application version v${rootVersion}`);
}

console.log(`Release version ${rootVersion} is consistent${tag ? ` with tag ${tag}` : ""}.`);
