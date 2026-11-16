// The toolchain that builds theme programs in the writer's tab (ARCHITECTURE.md R36), made into
// what Caprice serves: node toolchain/pack.mjs <dist> (web/build.sh runs it on web/dist). From
//   - the release of the investigation's toolchain that toolchain/release.json pins (rustc and
//     wasm-ld as WebAssembly, the compiler built from the commit rustup's 1.99.1 is built from,
//     and the VFS module they run on), every file checked against its pin: from
//     $CAPRICE_TOOLCHAIN_RELEASE if set, else as toolchain/fetch.mjs fetched it from GitHub, else
//     from the investigation's checkout beside this one;
//   - rustup's own 1.99.1 std for wasm32-unknown-unknown, without what no theme can reach (test,
//     getopts, proc_macro and their own dependencies), as two tars: the .rmeta files, the rest;
//   - the theme SDK (theme-sdk/) built as one rlib, as themes/build.sh builds it;
//   - the brotli decoder (toolchain/unbrotli),
// it writes <dist>/compiler/toolchain.json, naming the toolchain, and <dist>/toolchain/<id>/: each file as brotli, cut in
// pieces of at most 24 MiB (Workers' static assets take 25 MiB a file) named by their hash, the
// decoder and the VFS as they are, and manifest.json, which names them all with the flags every
// theme is built with. The id is the compiler's (RUSTC_ID) and a hash of everything else, so a new
// SDK, std or flag is a new toolchain, new files beside the old ones.
//
// Before writing, it builds every theme of themes/ natively, with rustup's 1.99.1 and exactly the
// packed sysroot, SDK and flags, and fails unless each is its fixture byte for byte: the tab's
// builds (bit-identical to native ones, the plan's step 0d) are then the fixtures too.
//
// Brotli at its best takes minutes for the sysroot: what it made is kept in target/toolchain, by
// the hash of what it was made of. Needs rustup's 1.99.1 with wasm32-unknown-unknown, and brotli.
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const dist = path.resolve(process.argv[2] ?? path.join(root, "web/dist"));
const work = path.join(root, "target/toolchain");
const pinned = JSON.parse(fs.readFileSync(path.join(root, "toolchain/release.json"), "utf8"));
const PIECE = 24 * 2 ** 20;
const RUSTC = "1.99.1";
const TARGET = "wasm32-unknown-unknown";

// Every theme is built with these, and nothing else (web/compiler/compiler.js adds the SDK, the
// source /theme/theme.rs and the output theme.wasm: wasm-ld writes the output's name into it).
const FLAGS = ["--crate-name", "theme", "--edition", "2024", "--target", TARGET, "-O", "-C", "codegen-units=1", "-C", "panic=abort",
  "-C", "strip=debuginfo", "--crate-type", "cdylib", "-C", "link-arg=--max-memory=33554432"];
const SDK = "/sdk/libtheme_sdk.rlib";
// The crates of std a theme cannot reach.
const UNREACHABLE = /^lib(test|getopts|proc_macro|rustc_literal_escaper|rustc_std_workspace_std|unicode_width)-/;

const sha = (b) => createHash("sha256").update(b).digest("hex");
const mib = (n) => (n / 2 ** 20).toFixed(1);
const run = (cmd, args, opts = {}) => execFileSync(cmd, args, { stdio: ["ignore", "pipe", "inherit"], maxBuffer: 2 ** 30, ...opts });
const env = { ...process.env, PATH: `${path.join(os.homedir(), ".cargo/bin")}:${process.env.PATH}` };
const rustc = (args, opts = {}) => run("rustc", [`+${RUSTC}`, ...args], { env, ...opts });

// Where each file of the release is: the folder given, the one fetch.mjs fills, or the checkout.
const investigation = path.join(os.homedir(), "Projects/rust-compiler-wasm-investigation");
const places = process.env.CAPRICE_TOOLCHAIN_RELEASE ? { all: process.env.CAPRICE_TOOLCHAIN_RELEASE }
  : fs.existsSync(path.join(work, "release", pinned.tag, "manifest.json")) ? { all: path.join(work, "release", pinned.tag) }
  : { all: path.join(investigation, "build/release", pinned.tag), "vfs.wasm": path.join(investigation, "build/www/orchestrator") };
const release = {};
for (const [name, want] of Object.entries(pinned.files)) {
  const file = path.join(places[name] ?? places.all, name);
  if (!fs.existsSync(file)) throw new Error(`the toolchain's ${name} is not at ${file}: run node toolchain/fetch.mjs`);
  const bytes = fs.readFileSync(file);
  if (sha(bytes) !== want) throw new Error(`${file} is not the file toolchain/release.json pins`);
  release[name] = bytes;
}
fs.mkdirSync(work, { recursive: true });
const tmp = fs.mkdtempSync(path.join(work, "pack-"));
try {
  // The SDK, as themes/build.sh builds it.
  rustc(["--edition", "2024", "--target", TARGET, "-O", "-C", "codegen-units=1", "-C", "panic=abort", "-C", "strip=debuginfo",
    "--crate-type", "rlib", "--crate-name", "theme_sdk", "--remap-path-prefix", `${root}/theme-sdk=/sdk`,
    path.join(root, "theme-sdk/src/lib.rs"), "-o", path.join(tmp, "libtheme_sdk.rlib")]);
  const sdk = fs.readFileSync(path.join(tmp, "libtheme_sdk.rlib"));

  // The sysroot, without the crates a theme cannot reach.
  const std = path.join(rustc(["--print", "sysroot"], { encoding: "utf8" }).trim(), "lib/rustlib", TARGET, "lib");
  const sysroot = path.join(tmp, "sysroot"), lib = path.join(sysroot, "lib/rustlib", TARGET, "lib");
  fs.mkdirSync(lib, { recursive: true });
  for (const f of fs.readdirSync(std)) if (!UNREACHABLE.test(f)) fs.copyFileSync(path.join(std, f), path.join(lib, f));

  // Every theme built natively with what is packed, and held to its fixture.
  fs.mkdirSync(path.join(tmp, "sdk"));
  fs.writeFileSync(path.join(tmp, "sdk/libtheme_sdk.rlib"), sdk);
  for (const name of fs.readdirSync(path.join(root, "themes"))) {
    const source = path.join(root, "themes", name, "theme.rs");
    if (!fs.existsSync(source)) continue;
    const dir = path.join(tmp, "built", name);
    fs.mkdirSync(dir, { recursive: true });
    rustc([...FLAGS, "--sysroot", sysroot, "--extern", `theme_sdk=${path.join(tmp, "sdk/libtheme_sdk.rlib")}`,
      "--remap-path-prefix", `${path.dirname(source)}=/theme`, source, "-o", path.join(dir, "theme.wasm")]);
    const fixture = path.join(root, "tests/fixtures", `${name}.wasm`);
    if (!fs.existsSync(fixture) || !fs.readFileSync(fixture).equals(fs.readFileSync(path.join(dir, "theme.wasm"))))
      throw new Error(`themes/${name} built with the packed toolchain is not tests/fixtures/${name}.wasm: run themes/build.sh, or make the two agree`);
  }

  // The sysroot's two groups as deterministic tars, then brotli, kept by what they were made of.
  const tar = (name, filter) => {
    const list = run("find", [`lib/rustlib/${TARGET}/lib`, "-type", "f", ...filter], { cwd: sysroot, encoding: "utf8" }).trim().split("\n").sort();
    fs.writeFileSync(path.join(tmp, `${name}.list`), list.join("\n") + "\n");
    run("tar", ["-C", sysroot, "--format=ustar", "--mtime=@0", "--owner=0", "--group=0", "--numeric-owner", "--mode=a+rX,u+w,go-w",
      "-cf", path.join(tmp, `${name}.tar`), "-T", path.join(tmp, `${name}.list`)]);
    return fs.readFileSync(path.join(tmp, `${name}.tar`));
  };
  const brotli = (raw) => {
    const kept = path.join(work, `${sha(raw)}.br`);
    if (!fs.existsSync(kept)) {
      const file = path.join(tmp, "raw");
      fs.writeFileSync(file, raw);
      run("brotli", ["-q", "11", "-f", "-o", `${kept}.part`, file]);
      fs.renameSync(`${kept}.part`, kept);
    }
    return fs.readFileSync(kept);
  };

  // The decoder.
  run("cargo", [`+${RUSTC}`, "build", "--offline", "--locked", "--release", "--target", TARGET, "-q"],
    { cwd: path.join(root, "toolchain/unbrotli"), env: { ...env, RUSTFLAGS: "", CARGO_TARGET_DIR: path.join(work, "unbrotli") } });
  const unbrotli = fs.readFileSync(path.join(work, "unbrotli", TARGET, "release/unbrotli.wasm"));

  // The files, then the id from all of them.
  const given = JSON.parse(release["manifest.json"]);
  const files = {}, pieces = new Map();
  const add = (name, size, sha256, br) => {
    const list = [];
    for (let at = 0; at < br.length; at += PIECE) {
      const piece = br.subarray(at, Math.min(at + PIECE, br.length)), h = sha(piece);
      list.push({ file: `${h.slice(0, 20)}.br`, size: piece.length, sha256: h });
      pieces.set(`${h.slice(0, 20)}.br`, piece);
    }
    files[name] = { size, sha256, brSize: br.length, pieces: list };
  };
  for (const name of ["rustc", "lld"]) {
    const f = given[name], br = release[f.file];
    if (sha(br) !== f.brSha256) throw new Error(`${f.file} is not the file its release's manifest names`);
    add(name, f.size, f.sha256, br);
  }
  for (const [name, filter] of [["meta", ["-name", "*.rmeta"]], ["link", ["!", "-name", "*.rmeta"]], ["sdk", null]]) {
    const raw = filter ? tar(name, filter) : sdk;
    add(name, raw.length, sha(raw), brotli(raw));
  }
  const plain = {};
  for (const [name, bytes] of [["unbrotli", unbrotli], ["vfs", release["vfs.wasm"]]]) {
    const h = sha(bytes);
    plain[name] = { file: `${name}-${h.slice(0, 16)}.wasm`, sha256: h };
    pieces.set(plain[name].file, bytes);
  }
  const made = { flags: FLAGS, sdk: SDK, target: TARGET, files: Object.fromEntries(Object.entries(files).map(([k, f]) => [k, f.sha256])), plain };
  const id = `${given.RUSTC_ID}-${sha(JSON.stringify(made)).slice(0, 8)}`;
  const manifest = { id, rustc: given.RUSTC_ID, version: given.version, target: TARGET, flags: FLAGS, sdk: SDK, files, plain };

  const to = path.join(dist, "toolchain", id);
  fs.mkdirSync(to, { recursive: true });
  for (const [file, bytes] of pieces) fs.writeFileSync(path.join(to, file), bytes);
  fs.writeFileSync(path.join(to, "manifest.json"), JSON.stringify(manifest, null, 1) + "\n");
  fs.mkdirSync(path.join(dist, "compiler"), { recursive: true });
  fs.writeFileSync(path.join(dist, "compiler/toolchain.json"), JSON.stringify({ id }) + "\n");
  const all = Object.values(files);
  console.log(`toolchain ${id}: ${all.reduce((a, f) => a + f.pieces.length, 0)} pieces, ${mib(all.reduce((a, f) => a + f.brSize, 0))} MiB `
    + `(${mib(all.reduce((a, f) => a + f.size, 0))} MiB decoded), into ${path.relative(root, to)}`);
} finally {
  fs.rmSync(tmp, { recursive: true, force: true });
}
