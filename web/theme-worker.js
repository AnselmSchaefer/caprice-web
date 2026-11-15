// A theme program's worker (src/platform/program.rs, ARCHITECTURE.md R36): the program runs here,
// in the browser's own engine, apart from Caprice, which ends this worker if it does not answer.
// Each frame Caprice sends the frame's input and the events as the canonical ABI lays them out
// (written by src/eggs/program.rs), and what `host` answers meanwhile; this host, written by hand
// for the one world caprice:theme@1.0.0, puts them into the program's memory, calls it, and lifts
// its `frame-output` into flat words, sent back in a buffer that Caprice gives back with the next
// frame. Never JavaScript objects: copying those cost more than the whole frame (step 0c of the
// plan). The words are the same as `platform::program::lift` makes (web/theme-worker.test.mjs).

const HOST = "caprice:theme/host@1.0.0";
// The head of the words: desk shapes, window shapes, over shapes, next, the title's offset (x, y),
// asked to lift, the ground.
const HEAD = 8;

// A theme loaded: `frame(ask, words)` runs the events and a frame, and gives back the words
// (grown as needed) and how many it wrote.
export async function load(bytes) {
  const module = await WebAssembly.compile(bytes);
  let e, ask = null, asked = false;
  const view = () => new DataView(e.memory.buffer);
  const imports = { [HOST]: {
    "story-seed": () => BigInt.asIntN(64, BigInt(ask.seed)),
    "page-count": () => ask.pages | 0,
    "ask-lift": () => { asked = true; return ask.lift ? 1 : 0; },
    "garden": (ret) => {
      const garden = ask.garden;
      const ptr = garden.length ? e.cabi_realloc(0, 0, 4, garden.length * 4) : 4;
      new Uint32Array(e.memory.buffer, ptr, garden.length).set(garden);
      const v = view();
      v.setUint32(ret, ptr, true);
      v.setUint32(ret + 4, garden.length, true);
    },
  } };
  e = (await WebAssembly.instantiate(module, imports)).exports;
  const give = (bytes, align) => {
    if (!bytes.byteLength) return align;
    const ptr = e.cabi_realloc(0, 0, align, bytes.byteLength);
    new Uint8Array(e.memory.buffer, ptr, bytes.byteLength).set(new Uint8Array(bytes));
    return ptr;
  };
  return (a, words) => {
    ask = a;
    asked = false;
    e.events(give(a.events, 4), a.events.byteLength / 12);
    const ret = e.frame(give(a.input, 8));
    const v = view();
    const size = v.byteLength;
    const list = (at, each) => {
      const ptr = v.getUint32(at, true), len = v.getUint32(at + 4, true);
      if (ptr + len * each > size) throw new Error("a list of its output lies outside its memory");
      return [ptr, len];
    };
    if (ret + 40 > size) throw new Error("its output lies outside its memory");
    const [desk, nd] = list(ret, 28), [win, nw] = list(ret + 8, 28), [over, no] = list(ret + 16, 28);
    let n = HEAD;
    const need = (k) => {
      if (n + k > words.length) {
        const more = new Uint32Array(Math.max(words.length * 2, n + k, 1024));
        more.set(words.subarray(0, n));
        words = more;
      }
    };
    need(0);
    words[0] = nd; words[1] = nw; words[2] = no;
    words[3] = v.getUint32(ret + 24, true);
    words[4] = v.getUint32(ret + 28, true); words[5] = v.getUint32(ret + 32, true);
    words[6] = asked ? 1 : 0; words[7] = v.getUint32(ret + 36, true);
    const shapes = (ptr, len) => {
      for (let i = 0; i < len; i++) {
        const b = ptr + 28 * i, p = b + 4;
        const kind = v.getUint8(b);
        if (kind === 0) {
          need(5);
          words[n] = 1; words[n + 1] = v.getUint32(p + 12, true);
          words[n + 2] = v.getUint32(p, true); words[n + 3] = v.getUint32(p + 4, true); words[n + 4] = v.getUint32(p + 8, true);
          n += 5;
        } else if (kind === 1) {
          const [q, m] = list(p, 8);
          need(3 + 2 * m);
          words[n] = 2; words[n + 1] = v.getUint32(p + 8, true); words[n + 2] = m;
          for (let k = 0; k < 2 * m; k++) words[n + 3 + k] = v.getUint32(q + 4 * k, true);
          n += 3 + 2 * m;
        } else if (kind === 2) {
          need(7);
          words[n] = 3; words[n + 1] = v.getUint32(p + 20, true); words[n + 2] = v.getUint32(p + 16, true);
          for (let k = 0; k < 4; k++) words[n + 3 + k] = v.getUint32(p + 4 * k, true);
          n += 7;
        } else if (kind === 3) {
          const [qv, nv] = list(p, 12), [qi, ni] = list(p + 8, 4);
          need(3 + 3 * nv + ni);
          words[n] = 4; words[n + 1] = nv; words[n + 2] = ni;
          for (let k = 0; k < 3 * nv; k++) words[n + 3 + k] = v.getUint32(qv + 4 * k, true);
          n += 3 + 3 * nv;
          for (let k = 0; k < ni; k++) words[n + k] = v.getUint32(qi + 4 * k, true);
          n += ni;
        } else if (kind === 4) {
          const [q, m] = list(p, 8);
          need(5 + 2 * m);
          words[n] = 5; words[n + 1] = v.getUint32(p + 16, true); words[n + 2] = v.getUint32(p + 12, true);
          words[n + 3] = v.getUint8(p + 8); words[n + 4] = m;
          for (let k = 0; k < 2 * m; k++) words[n + 5 + k] = v.getUint32(q + 4 * k, true);
          n += 5 + 2 * m;
        } else {
          throw new Error("a shape of no kind");
        }
      }
    };
    shapes(desk, nd);
    shapes(win, nw);
    shapes(over, no);
    e.cabi_post_frame(ret);
    ask = null;
    return { words, n };
  };
}

// In a worker: loaded from the first message, a frame for each one after.
if (typeof WorkerGlobalScope !== "undefined" && self instanceof WorkerGlobalScope) {
  let frame = null, words = new Uint32Array(1 << 16), failed = false;
  const stop = (why) => {
    failed = true;
    postMessage({ stopped: why });
  };
  self.onmessage = async ({ data }) => {
    if (failed) return;
    try {
      if (data.load) {
        frame = await load(data.load);
        postMessage({ loaded: true });
        return;
      }
      if (data.buffer && data.buffer.byteLength >= words.byteLength) words = new Uint32Array(data.buffer);
      const made = frame(data, words);
      postMessage({ words: made.words.buffer, n: made.n }, [made.words.buffer]);
      words = new Uint32Array(0);
    } catch (err) {
      stop(String((err && err.message) || err));
    }
  };
}
