// `buf breaking` against a git ref (default origin/main), allowing one break:
// the rename of the proto packages from `hermes.*.v1` to `flotilla.*.v1`.
// Binary frames carry field numbers and JSON carries field names, never a
// package, so that rename is wire-compatible (docs/protocol.md, "Renames").
// When the ref still has `proto/hermes/`, its files are compared as if moved
// to `proto/flotilla/` with their packages renamed; every other change is
// judged as usual.
import { execFileSync, spawnSync } from "node:child_process";
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const windows = process.platform === "win32";
const buf = join(root, "apps", "desktop", "node_modules", ".bin", windows ? "buf.cmd" : "buf");
const ref = process.argv[2] ?? "origin/main";

function files(dir) {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    return statSync(path).isDirectory() ? files(path) : [path];
  });
}

/** The ref's protos with the pre-rename packages spelled as today's. */
export function renamePackages(text) {
  return text
    .replace(/\bhermes\.(board|home|pr|wire)\b/g, "flotilla.$1")
    .replace(/(["/])hermes\/(board|home|pr|wire)\//g, "$1flotilla/$2/")
    .replace(/^(\s*-\s*)proto\/hermes\//gm, "$1proto/flotilla/");
}

if (spawnSync("git", ["cat-file", "-e", `${ref}:buf.yaml`], { cwd: root }).status !== 0) {
  process.stdout.write(`${ref} has no protos yet; nothing to compare\n`);
  process.exit(0);
}
const base = mkdtempSync(join(tmpdir(), "flotilla-buf-base-"));
try {
  const archive = execFileSync("git", ["archive", "--format=tar", ref, "buf.yaml", "proto"], {
    cwd: root,
    maxBuffer: 64 * 1024 * 1024,
  });
  execFileSync("tar", ["-x", "-C", base], { input: archive });
  const old = join(base, "proto", "hermes");
  if (existsSync(old) && !existsSync(join(base, "proto", "flotilla"))) {
    renameSync(old, join(base, "proto", "flotilla"));
    for (const file of [...files(join(base, "proto")), join(base, "buf.yaml")]) {
      writeFileSync(file, renamePackages(readFileSync(file, "utf8")));
    }
    process.stdout.write(`comparing with ${ref}'s hermes.*.v1 read as flotilla.*.v1\n`);
  }
  const result = spawnSync(buf, ["breaking", "--against", base], {
    cwd: root,
    stdio: "inherit",
    shell: windows,
  });
  process.exitCode = result.status ?? 1;
} finally {
  rmSync(base, { recursive: true, force: true });
}
