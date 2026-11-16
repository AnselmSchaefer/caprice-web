// The Worker's check of a theme's module before it is queued for the gallery (R36, the plan's
// step 6), by reading its sections and never running it: within the size, imports only
// `caprice:theme/host@1.0.0` and only its functions with their signatures, exports exactly what the
// world needs with its signatures, one memory of its own (not shared, not imported, at most 32 MiB
// declared), and every instruction of every function within the fixed set of features (WebAssembly
// 2.0 without SIMD: bulk memory, sign extension, multi-value, mutable globals, reference types,
// saturating float to int). It is the app's `eggs::program::check` made cheap enough for the free
// plan's 10 ms (0.2 ms for the Snow's module, every instruction read: the plan's step 0d); whether
// the module is valid WebAssembly is the app's to say, when the owner's browser plays it. So a
// change to what the app's check allows is made here too. Returns { ok, why: [...] }: every reason.

// The app's MOST_BYTES and MOST_PAGES (src/eggs/program.rs), and a bound on the functions read.
export const LIMITS = { bytes: 1 << 20, pages: 512, functions: 20_000 };
const HOST = "caprice:theme/host@1.0.0";
const I32 = 0x7f, I64 = 0x7e, F32 = 0x7d, F64 = 0x7c, FUNCREF = 0x70, EXTERNREF = 0x6f;
const sig = (params, results) => `${params.map(t => t.toString(16)).join(",")}>${results.map(t => t.toString(16)).join(",")}`;
// What the world's core module imports and exports, as the canonical ABI flattens it.
const IMPORTS = {
  "story-seed": sig([], [I64]), "garden": sig([I32], []), "page-count": sig([], [I32]), "ask-lift": sig([], [I32]),
};
const EXPORTS = {
  "events": sig([I32, I32], []), "frame": sig([I32], [I32]), "cabi_post_frame": sig([I32], []),
  "cabi_realloc": sig([I32, I32, I32, I32], [I32]),
};

class Reader {
  constructor(b, at = 0, end = b.length) { this.b = b; this.at = at; this.end = end; }
  byte() { if (this.at >= this.end) throw new Error("ends early"); return this.b[this.at++]; }
  u32() {
    let r = 0, shift = 0, x;
    do { x = this.byte(); r |= (x & 0x7f) << shift; shift += 7; } while (x & 0x80 && shift < 35);
    if (x & 0x80) throw new Error("a number too long");
    return r >>> 0;
  }
  // A signed LEB of up to 64 bits, skipped (its value is never needed).
  sleb(max) { let n = 0, x; do { x = this.byte(); n++; } while (x & 0x80 && n <= max); if (x & 0x80) throw new Error("a number too long"); }
  skip(n) { if (this.at + n > this.end) throw new Error("ends early"); this.at += n; }
  name() { const n = this.u32(), s = this.at; this.skip(n); return new TextDecoder("utf-8", { fatal: true }).decode(this.b.subarray(s, this.at)); }
  limits() {
    const flags = this.byte(), initial = this.u32();
    return { flags, initial, maximum: flags & 1 ? this.u32() : undefined };
  }
}

// The single-byte opcodes allowed, and which immediates each has (a byte table: the scan of
// every instruction is the check's only cost that grows with the module, so it is kept tight).
const NONE = 0, U32 = 1, BLOCK = 2, BR_TABLE = 3, CALL_IND = 4, MEM = 5, LEB = 6, F32C = 7, F64C = 8, SELECT_T = 9, MEMIDX = 10, REFT = 11, FC = 12, NO = 255;
const OPS = new Uint8Array(256).fill(NO);
const put = (from, to, kind) => OPS.fill(kind, from, to + 1);
put(0x00, 0x01, NONE);              // unreachable, nop
put(0x02, 0x04, BLOCK);             // block, loop, if (multi-value block types allowed)
put(0x05, 0x05, NONE); put(0x0b, 0x0b, NONE); // else, end
put(0x0c, 0x0d, U32);               // br, br_if
put(0x0e, 0x0e, BR_TABLE);
put(0x0f, 0x0f, NONE);              // return
put(0x10, 0x10, U32);               // call
put(0x11, 0x11, CALL_IND);          // call_indirect
put(0x1a, 0x1b, NONE);              // drop, select
put(0x1c, 0x1c, SELECT_T);          // select t* (reference types)
put(0x20, 0x26, U32);               // local.get/set/tee, global.get/set, table.get/set
put(0x28, 0x3e, MEM);               // loads and stores
put(0x3f, 0x40, MEMIDX);            // memory.size, memory.grow
put(0x41, 0x42, LEB);               // i32.const, i64.const
put(0x43, 0x43, F32C); put(0x44, 0x44, F64C);
put(0x45, 0xc4, NONE);              // numeric, conversions, sign extension (0xc0-0xc4)
put(0xd0, 0xd0, REFT);              // ref.null t
put(0xd1, 0xd1, NONE);              // ref.is_null
put(0xd2, 0xd2, U32);               // ref.func
put(0xfc, 0xfc, FC);                // saturating truncation (0-7), bulk memory (8-11), table ops (12-17)
// Named for the reasons given when something else is found.
const FEATURE = {
  0x06: "exceptions", 0x07: "exceptions", 0x08: "exceptions", 0x09: "exceptions", 0x0a: "exceptions", 0x18: "exceptions",
  0x19: "exceptions", 0x1f: "exceptions", 0x12: "tail calls", 0x13: "tail calls", 0x14: "function references",
  0x15: "function references", 0xd3: "function references", 0xd4: "function references", 0xd5: "function references",
  0xd6: "function references", 0xfb: "garbage collection", 0xfd: "SIMD", 0xfe: "threads",
};
const VALUE = new Set([I32, I64, F32, F64, FUNCREF, EXTERNREF]);

/** Every instruction of one function body, b[at..end]; the first feature outside the set, or null. */
function scanBody(b, at, end) {
  // A LEB number is skipped by its bytes; its value is needed only for counts.
  const u32 = () => { let r = 0, shift = 0, x; do { x = b[at++]; r |= (x & 0x7f) << shift; shift += 7; } while (x & 0x80); return r >>> 0; };
  for (let n = u32(); n > 0; n--) { u32(); const t = b[at++]; if (!VALUE.has(t)) return `a local of type 0x${t.toString(16)}`; }
  while (at < end) {
    const op = b[at++];
    switch (OPS[op]) {
      case NONE: break;
      case U32: case LEB: while (b[at++] & 0x80); break;
      case BLOCK: { const t = b[at]; if (t === 0x40 || VALUE.has(t)) at++; else while (b[at++] & 0x80); break; }
      case BR_TABLE: for (let n = u32(); n >= 0; n--) while (b[at++] & 0x80); break;
      case CALL_IND: while (b[at++] & 0x80); while (b[at++] & 0x80); break;
      case MEM: if (b[at] & 0x40) return "multiple memories"; while (b[at++] & 0x80); while (b[at++] & 0x80); break;
      case MEMIDX: if (b[at++] !== 0) return "multiple memories"; break;
      case F32C: at += 4; break;
      case F64C: at += 8; break;
      case SELECT_T: at += 1 + u32(); break;
      case REFT: at++; break;
      case FC: {
        const sub = u32();
        if (sub > 17) return `opcode 0xfc ${sub}`;
        if (sub === 8) { while (b[at++] & 0x80); if (b[at++] !== 0) return "multiple memories"; }      // memory.init
        else if (sub === 10) { if (b[at++] !== 0 || b[at++] !== 0) return "multiple memories"; }      // memory.copy
        else if (sub === 11) { if (b[at++] !== 0) return "multiple memories"; }                        // memory.fill
        else if (sub === 12 || sub === 14) { while (b[at++] & 0x80); while (b[at++] & 0x80); }         // table.init, table.copy
        else if (sub >= 9) while (b[at++] & 0x80);                                                     // data.drop, elem.drop, table.*
        break;
      }
      default: return FEATURE[op] ?? `opcode 0x${op.toString(16)}`;
    }
  }
  return at === end ? null : "an instruction past its function's end";
}

// A constant expression as the fixed set allows: one constant (or global.get, ref.null, ref.func), then end.
function constExpr(r) {
  const op = r.byte();
  if (op === 0x41) r.sleb(5); else if (op === 0x42) r.sleb(10); else if (op === 0x43) r.skip(4); else if (op === 0x44) r.skip(8);
  else if (op === 0x23 || op === 0xd2) r.u32(); else if (op === 0xd0) r.byte();
  else return `a constant expression with 0x${op.toString(16)} (extended constants)`;
  return r.byte() === 0x0b ? null : "a constant expression of more than one instruction (extended constants)";
}

export function checkModule(bytes) {
  const why = [], seen = { imports: [], exports: [], memory: null, functions: 0, codeBytes: 0 };
  const b = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
  if (b.length > LIMITS.bytes) why.push(`${b.length} bytes, more than ${LIMITS.bytes}`);
  if (b.length < 8 || b[0] !== 0 || b[1] !== 0x61 || b[2] !== 0x73 || b[3] !== 0x6d || b[4] !== 1 || b[5] || b[6] || b[7])
    return { ok: false, why: [...why, "not a WebAssembly module (version 1)"] };
  const types = [], funcs = []; // a function's type index, imported ones first
  let imported = 0, memories = 0;
  try {
    const r = new Reader(b, 8);
    while (r.at < r.end) {
      const id = r.byte(), size = r.u32(), end = r.at + size;
      if (end > b.length) throw new Error("a section past the end");
      // (Whether it is valid WebAssembly is the engine's to say, when the app loads it.)
      const s = new Reader(b, r.at, end);
      switch (id) {
        case 1: { // types
          for (let n = s.u32(), i = 0; i < n; i++) {
            const form = s.byte();
            if (form !== 0x60) { why.push(`a type of form 0x${form.toString(16)} (garbage collection)`); break; }
            const p = Array.from({ length: s.u32() }, () => s.byte()), q = Array.from({ length: s.u32() }, () => s.byte());
            types.push(sig(p, q));
          }
          break;
        }
        case 2: { // imports
          for (let n = s.u32(), i = 0; i < n; i++) {
            const module = s.name(), name = s.name(), kind = s.byte();
            seen.imports.push(`${module}.${name}`);
            if (kind === 0) { const t = s.u32(); funcs.push(t); imported++;
              if (module !== HOST || !(name in IMPORTS)) why.push(`imports ${module}.${name}`);
              else if (types[t] !== IMPORTS[name]) why.push(`imports ${name} with another signature`);
            } else {
              why.push(`imports ${module}.${name}, a ${["function", "table", "memory", "global", "tag"][kind] ?? "thing"}`);
              if (kind === 1) { s.byte(); s.limits(); } else if (kind === 2) { s.limits(); memories++; } else if (kind === 3) { s.byte(); s.byte(); } else s.skip(2);
            }
          }
          break;
        }
        case 3: for (let n = s.u32(), i = 0; i < n; i++) funcs.push(s.u32()); seen.functions = funcs.length - imported; break;
        case 4: for (let n = s.u32(), i = 0; i < n; i++) { const t = s.byte(); if (t !== FUNCREF && t !== EXTERNREF) why.push("a table of another type"); const l = s.limits(); if (l.flags & ~1) why.push("a 64-bit or shared table"); } break;
        case 5: { // memories
          for (let n = s.u32(), i = 0; i < n; i++) {
            const l = s.limits();
            memories++;
            seen.memory = { initial: l.initial, maximum: l.maximum };
            if (l.flags & 2) why.push("a shared memory (threads)");
            if (l.flags & 4) why.push("a 64-bit memory (memory64)");
            if (l.maximum === undefined) why.push("a memory with no maximum declared");
            else if (l.maximum > LIMITS.pages) why.push(`a memory of up to ${l.maximum} pages, more than ${LIMITS.pages} (32 MiB)`);
          }
          break;
        }
        case 6: for (let n = s.u32(), i = 0; i < n; i++) { s.byte(); s.byte(); const e = constExpr(s); if (e) why.push(e); } break;
        case 7: { // exports
          for (let n = s.u32(), i = 0; i < n; i++) {
            const name = s.name(), kind = s.byte(), index = s.u32();
            seen.exports.push(name);
            if (kind === 0 && name in EXPORTS) { if (types[funcs[index]] !== EXPORTS[name]) why.push(`exports ${name} with another signature`); }
            else if (!(kind === 2 && name === "memory")) why.push(`exports ${name}, which the world does not have`);
          }
          for (const name of [...Object.keys(EXPORTS), "memory"]) if (!seen.exports.includes(name)) why.push(`does not export ${name}`);
          break;
        }
        case 8: s.u32(); break; // start: allowed, it runs with nothing to call but host
        case 10: { // code: every instruction
          seen.codeBytes = size;
          const n = s.u32();
          if (n > LIMITS.functions) why.push(`${n} functions, more than ${LIMITS.functions}`);
          for (let i = 0; i < n; i++) {
            const len = s.u32();
            if (s.at + len > end) throw new Error("a function past its section");
            const bad = scanBody(b, s.at, s.at + len);
            if (bad) { why.push(`function ${imported + i} uses ${bad}`); if (why.length > 20) break; }
            s.skip(len);
          }
          break;
        }
        case 13: why.push("a tag section (exceptions)"); break;
        case 0: case 9: case 11: case 12: break; // custom, elements, data, data count
        default: why.push(`section ${id}, unknown`);
      }
      r.at = end;
    }
  } catch (e) {
    why.push(`malformed: ${e.message}`);
  }
  if (memories !== 1) why.push(`${memories} memories, not one`);
  return { ok: why.length === 0, why, ...seen };
}
