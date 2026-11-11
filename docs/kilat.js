// Kilat for the web: check and run Kilat programs in a WebAssembly sandbox.
// This file plus kilat.wasm is everything. No dependencies and no build step.
//
//   import { loadKilat } from "./kilat.js";
//
//   const kilat = await loadKilat();              // fetches kilat.wasm from next to this file
//   const result = await kilat.run('print("stress =", 250 N / (4 mm * 3 mm) in MPa)');
//   result.output                                 // "stress = 20.8333333333\n"
//
//   const bad = await kilat.check("let gap: mm = 2 s");
//   bad.ok                                        // false: a unit error, found without running
//   bad.error                                     // the message, with the line and a marker
//
// A result is { ok, output, error, kind, fuelUsed }. `kind` is "" when ok, otherwise
// "compile", "runtime", "fuel", "memory", "stopped" or "crash".
//
// A program runs in a sandbox. It has a fuel limit and a memory limit, and it cannot reach
// files, the network, the page or anything else unless you pass a `host` function:
//
//   const host = async (name, payload) => {
//     if (name === "http_get") return (await fetch(payload)).text();
//     return null;                                // everything else stays refused
//   };
//   await kilat.run(source, { host });
//
// `host` returns a string to answer Ok(text), returns null to refuse, or throws to answer
// Err(message). It may be async. The requests a program can make are read_file, write_file
// (payload "path\ntext"), http_get, ask_ai, input, clock, time and args.
//
// Options for run():
//   fuel       the most loop turns and function calls the program may make (default 200 million)
//   memoryMb   the most memory it may use, in MB (default 64)
//   host       see above
//   files      { "name.kilat": "source" } made available to `import "name.kilat"`
//   seed       a number that makes rand() repeatable (default: a new one for every run)
//   printLast  also print the value of a final expression, like a calculator
//   onOutput   called with each piece of printed text while the program runs
//   signal     an AbortSignal; aborting it stops the program
//
// check() takes { files }.
//
// To run programs off the page's thread, ask for a worker:
//
//   const kilat = await loadKilat(undefined, { worker: true });
//   kilat.inWorker                                // true when a worker could be started
//
// run() then works the same, with the same options and results, but the page stays smooth
// even during one long step, and aborting the signal stops the program at once. The price
// is speed: in headless Edge the same program took 1.5 to 2.5 times as long in a worker.
// Where a worker cannot start (an old browser, a strict content policy, a script from
// another site) programs run on the page as before and inWorker is false. check() always
// runs on the page: it is quick.

const ERROR_KINDS = ["", "compile", "runtime", "fuel", "memory"];

const OUTPUT = 0;
const ERROR = 1;
const HOST_NAME = 2;
const HOST_PAYLOAD = 3;
const VERSION = 4;

const DONE = 0;
const HOST_CALL = 1;
const MORE = 2;
const FAILED = 3;

const MODE_RUN = 0;
const MODE_CHECK = 1;
const MODE_PRINT_LAST = 2;

const READY = 0;
const FIRST_SLICE = 10; // loop turns and calls in the first step
const LARGEST_SLICE = 200000;
const STEP_MS = 1; // how long one step should take
const FRAME_MS = 40; // compute this long, then let the page handle clicks and repaint
const BYTES_PER_MB = 1048576;
const WORKER_NAME = "kilat-worker";
const WORKER_START_MS = 4000; // a worker that is not ready by then is given up on

const encoder = new TextEncoder();
const decoder = new TextDecoder();

// `wasm` may be a URL, the bytes (ArrayBuffer or typed array) or a WebAssembly.Module.
// `settings.worker` asks for programs to run in a worker (see the top of this file).
export async function loadKilat(wasm, settings = {}) {
  const module = await compileWasm(wasm ?? new URL("kilat.wasm", import.meta.url));
  const { api, session } = await openSession(module);
  const version = readText(api, session, VERSION);
  // One idle worker is kept warm, so a run does not wait for a worker to start.
  const pool = { spare: settings.worker ? await startWorker() : null };
  const inWorker = pool.spare !== null;
  return {
    version,
    inWorker,
    run(source, options = {}) {
      const mode = options.printLast ? MODE_PRINT_LAST : MODE_RUN;
      return inWorker
        ? executeInWorker(pool, module, source, mode, options)
        : execute(module, source, mode, options);
    },
    check(source, options = {}) {
      return execute(module, source, MODE_CHECK, { files: options.files });
    },
  };
}

async function compileWasm(wasm) {
  if (wasm instanceof WebAssembly.Module) {
    return wasm;
  }
  if (wasm instanceof ArrayBuffer || ArrayBuffer.isView(wasm)) {
    return WebAssembly.compile(wasm);
  }
  const response = await fetch(wasm);
  if (!response.ok) {
    throw new Error(`could not load ${response.url} (status ${response.status})`);
  }
  return WebAssembly.compile(await response.arrayBuffer());
}

// Every run gets its own instance: a clean memory each time, and a crash cannot leak into
// the next run.
async function openSession(module) {
  const instance = await WebAssembly.instantiate(module, {});
  const api = instance.exports;
  return { api, session: api.kilat_session_new() };
}

function readText(api, session, which) {
  const length = api.kilat_text_len(session, which) >>> 0;
  if (length === 0) {
    return "";
  }
  const pointer = api.kilat_text(session, which) >>> 0;
  return decoder.decode(new Uint8Array(api.memory.buffer, pointer, length));
}

// Copies `text` into the sandbox, calls use(pointer, length), then frees the copy.
function withText(api, text, use) {
  const bytes = encoder.encode(String(text));
  const pointer = api.kilat_alloc(bytes.length) >>> 0;
  new Uint8Array(api.memory.buffer, pointer, bytes.length).set(bytes);
  try {
    return use(pointer, bytes.length);
  } finally {
    api.kilat_dealloc(pointer, bytes.length);
  }
}

// A loop turn can take a nanosecond or (copying a long text) many milliseconds, so the
// number of turns per step follows the clock. That keeps the page responsive and lets a
// stop signal through, however much work the program does in one turn.
function nextSlice(slice, milliseconds) {
  const wanted = milliseconds > 0 ? (slice * STEP_MS) / milliseconds : slice * 8;
  return Math.max(1, Math.min(Math.round(wanted), slice * 8, LARGEST_SLICE));
}

// One trip through the event loop. A message channel is not slowed down in background
// tabs the way timers are.
function pause() {
  return new Promise((resolve) => {
    const channel = new MessageChannel();
    channel.port1.onmessage = () => {
      channel.port1.close();
      resolve();
    };
    channel.port2.postMessage(0);
  });
}

async function answer(host, name, payload, startedAt) {
  if (host) {
    try {
      const reply = await host(name, payload);
      if (reply !== null && reply !== undefined) {
        return [true, String(reply)];
      }
    } catch (problem) {
      // the program gets Err(message) and decides what to do
      return [false, String((problem && problem.message) || problem || "error")];
    }
  }
  if (name === "clock") {
    return [true, ((performance.now() - startedAt) / 1000).toFixed(6)];
  }
  if (name === "time") {
    return [true, (Date.now() / 1000).toFixed(3)];
  }
  if (name === "args") {
    return [true, ""];
  }
  return [false, `\`${name}\` is not allowed here`];
}

function isTrap(problem) {
  return problem instanceof WebAssembly.RuntimeError || problem instanceof RangeError;
}

async function execute(module, source, mode, options) {
  const startedAt = performance.now();
  const pieces = [];
  const finish = (ok, error, kind, fuelUsed) => ({ ok, output: pieces.join(""), error, kind, fuelUsed });
  try {
    const { api, session } = await openSession(module);
    for (const [name, content] of Object.entries(options.files ?? {})) {
      withText(api, name, (namePointer, nameLength) =>
        withText(api, content, (pointer, length) =>
          api.kilat_add_file(session, namePointer, nameLength, pointer, length)));
    }
    api.kilat_set_seed(session, options.seed ?? Math.floor(Math.random() * 2 ** 52));
    const fuel = Number(options.fuel ?? 0);
    const memoryBytes = Number(options.memoryMb ?? 0) * BYTES_PER_MB;
    let state = withText(api, source, (pointer, length) =>
      api.kilat_start(session, pointer, length, mode, fuel, memoryBytes));

    let slice = FIRST_SLICE;
    let lastPause = performance.now();
    while (state === READY) {
      let now = performance.now();
      let step = MORE;
      while (step === MORE && now - lastPause < FRAME_MS) {
        step = api.kilat_step(session, slice);
        const stepEnd = performance.now();
        slice = nextSlice(slice, stepEnd - now);
        now = stepEnd;
      }
      const piece = readText(api, session, OUTPUT);
      if (piece) {
        pieces.push(piece);
        api.kilat_clear_output(session);
        if (options.onOutput) {
          options.onOutput(piece);
        }
      }
      if (step === DONE) {
        break;
      }
      if (step === FAILED) {
        state = FAILED;
        break;
      }
      if (step === HOST_CALL) {
        const name = readText(api, session, HOST_NAME);
        const payload = readText(api, session, HOST_PAYLOAD);
        const [ok, reply] = await answer(options.host, name, payload, startedAt);
        withText(api, reply, (pointer, length) => api.kilat_resume(session, ok ? 1 : 0, pointer, length));
      }
      // The page gets its turn every FRAME_MS, also when the program keeps asking the host:
      // an answer that is ready at once would otherwise never let a timer or a click through.
      if (performance.now() - lastPause >= FRAME_MS) {
        if (options.onFrame) {
          // In a worker there is no page to let through: report and carry on.
          options.onFrame(api.kilat_fuel_used(session));
        } else {
          await pause();
        }
        lastPause = performance.now();
      }
      if (options.signal && options.signal.aborted) {
        return finish(false, "stopped", "stopped", api.kilat_fuel_used(session));
      }
    }

    const fuelUsed = api.kilat_fuel_used(session);
    if (state === READY) {
      return finish(true, "", "", fuelUsed);
    }
    const kind = ERROR_KINDS[api.kilat_error_kind(session)] || "runtime";
    return finish(false, readText(api, session, ERROR), kind, fuelUsed);
  } catch (problem) {
    if (!isTrap(problem)) {
      throw problem;
    }
    // The sandbox itself failed (for example on code nested thousands of levels deep).
    // Nothing leaks: the next run starts from a new instance.
    return finish(false, `internal error: ${problem.message}`, "crash", 0);
  }
}

// ---- worker mode ----
// This same file is the worker's script: a worker started under WORKER_NAME serves runs
// (see the last lines), and the page side below talks to it.

// A worker that has said it is ready, or null when one cannot be started here.
function startWorker() {
  return new Promise((resolve) => {
    let worker;
    try {
      worker = new Worker(import.meta.url, { type: "module", name: WORKER_NAME });
    } catch (problem) {
      resolve(null);
      return;
    }
    const giveUp = () => {
      clearTimeout(timer);
      worker.terminate();
      resolve(null);
    };
    const timer = setTimeout(giveUp, WORKER_START_MS);
    worker.onerror = giveUp;
    worker.onmessage = (event) => {
      if (event.data && event.data.type === "ready") {
        clearTimeout(timer);
        worker.onerror = null;
        worker.onmessage = null;
        resolve(worker);
      }
    };
  });
}

// The same as execute(), with the program running in a worker. Stopping ends the worker, so
// it works at once, also in the middle of one long step.
async function executeInWorker(pool, module, source, mode, options) {
  const worker = pool.spare ?? (await startWorker());
  pool.spare = null;
  if (worker === null) {
    return execute(module, source, mode, options);
  }
  const pieces = [];
  let fuelUsed = 0;
  return new Promise((resolve, reject) => {
    const signal = options.signal;
    let settled = false;
    const settle = (keepWorker, result, failure) => {
      settled = true;
      if (signal) {
        signal.removeEventListener("abort", stop);
      }
      worker.onmessage = null;
      worker.onerror = null;
      if (keepWorker && pool.spare === null) {
        pool.spare = worker;
      } else {
        worker.terminate();
      }
      if (failure) {
        reject(failure);
      } else {
        resolve(result);
      }
    };
    function stop() {
      settle(false, { ok: false, output: pieces.join(""), error: "stopped", kind: "stopped", fuelUsed });
    }
    const replyToHost = async (request) => {
      const reply = { type: "reply", id: request.id, ok: true, text: null };
      try {
        const text = await options.host(request.name, request.payload);
        reply.text = text === null || text === undefined ? null : String(text);
      } catch (problem) {
        reply.ok = false;
        reply.text = String((problem && problem.message) || problem || "error");
      }
      if (!settled) {
        worker.postMessage(reply);
      }
    };
    worker.onerror = (event) => settle(false, null, new Error(event.message || "the Kilat worker failed"));
    worker.onmessage = (event) => {
      const message = event.data;
      if (message.type === "output") {
        pieces.push(message.piece);
        if (options.onOutput) {
          options.onOutput(message.piece);
        }
      } else if (message.type === "frame") {
        fuelUsed = message.fuelUsed;
      } else if (message.type === "host") {
        replyToHost(message);
      } else if (message.type === "done") {
        settle(true, message.result);
      } else if (message.type === "failed") {
        settle(false, null, new Error(message.message));
      }
    };
    if (signal) {
      if (signal.aborted) {
        stop();
        return;
      }
      signal.addEventListener("abort", stop, { once: true });
    }
    worker.postMessage({
      type: "run",
      module,
      source: String(source),
      mode,
      hasHost: Boolean(options.host),
      options: { files: options.files, seed: options.seed, fuel: options.fuel, memoryMb: options.memoryMb },
    });
  });
}

// The worker's side: runs one program per "run" message and passes host requests to the page.
function serveRuns(scope) {
  const waiting = new Map();
  let lastId = 0;
  const askPage = (name, payload) =>
    new Promise((resolve, reject) => {
      lastId += 1;
      waiting.set(lastId, { resolve, reject });
      scope.postMessage({ type: "host", id: lastId, name, payload });
    });
  scope.onmessage = async (event) => {
    const message = event.data;
    if (message.type === "reply") {
      const asker = waiting.get(message.id);
      waiting.delete(message.id);
      if (asker && message.ok) {
        asker.resolve(message.text);
      } else if (asker) {
        asker.reject(new Error(message.text));
      }
      return;
    }
    try {
      const result = await execute(message.module, message.source, message.mode, {
        ...message.options,
        host: message.hasHost ? askPage : undefined,
        onOutput: (piece) => scope.postMessage({ type: "output", piece }),
        onFrame: (fuelUsed) => scope.postMessage({ type: "frame", fuelUsed }),
      });
      scope.postMessage({ type: "done", result });
    } catch (problem) {
      scope.postMessage({ type: "failed", message: String((problem && problem.message) || problem) });
    }
  };
  scope.postMessage({ type: "ready" });
}

if (typeof WorkerGlobalScope !== "undefined" && self instanceof WorkerGlobalScope && self.name === WORKER_NAME) {
  serveRuns(self);
}
