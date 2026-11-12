# Kilat

Kilat is a small programming language for engineering calculations and safe scripts.
Units and types are checked before a program runs, so `2 m + 3 kg` is refused instead of
printing a number. The whole language is one program of 1.3 MB with no dependencies.
The same engine also runs in a web page, inside Python, and as a plugin for the Cairn notes app.
"Kilat" is the Malay word for lightning.

- Try it in the browser: <https://juw12.github.io/kilat/>
- Download for Windows: <https://github.com/juw12/kilat/releases/latest>
- Language guide: [docs/LANGUAGE.md](docs/LANGUAGE.md)
- Measured results: [RESULTS.md](RESULTS.md)

```kilat
let force = 1200 N
let area = 58 mm^2
let stress = force / area
print("stress = {stress:MPa.1}")
```

```text
stress = 20.7 MPa
```

The kind of mistake it is built to stop:

```kilat
let length = 2 m
let mass = 3 kg
print(length + mass)
```

```text
error: unit mismatch: cannot add m and kg
  --> example.kilat:3:14
  |
3 | print(length + mass)
  |              ^
```

## Is it better than Python?

Not overall. Python has the libraries, the tools and the community, and Kilat has none of that.
Kilat is aimed at one job: calculations and small scripts that have to be right and have to be
safe to run, including code written by an AI model. The numbers below show where it is ahead on
that job and where it is behind. A script copied them from `judge_report.json`, which
`python judge.py` writes. Measured on 2026-11-12 with kilat 0.2.1 and CPython 3.13.2
on Windows 10, Intel(R) Core(TM) Ultra 7 258V.

### Mistakes found before the program runs

`bugs/` holds 20 small programs, each with one realistic mistake, written in Kilat and in Python.

| Kind of mistake | Programs | Kilat | mypy --strict | CPython alone |
|---|---|---|---|---|
| units | 5 | 5 | 0 | 0 |
| names | 4 | 4 | 4 | 0 |
| types | 6 | 6 | 6 | 0 |
| safety | 5 | 5 | 4 | 0 |
| **all** | 20 | 20 | 14 | 0 |

Run anyway, 10 of the Python versions stop with an error and 10 print a wrong or meaningless answer.
The author of Kilat wrote these programs, so read the table as a demonstration of what unit
checking adds, not as an independent benchmark.

### Numbers

| Case | Kilat | Python |
|---|---|---|
| `round_to(2.675, 2)` | 2.68 | 2.67 |
| `"{1.005:.2}"` | 1.01 | 1.00 |
| `0.0 / 0.0` | runtime error: division by zero | ZeroDivisionError |
| `sqrt(-1.0)` | runtime error: sqrt needs a number that is not negative | ValueError |
| `9223372036854775807 + 1` | runtime error: integer overflow | 9223372036854775808 |
| `a misspelt name` | did you mean `length`? | NameError when the line runs |

Not trapped: `1e308 * 10.0` gives `inf`, and infinity minus infinity gives `nan`.

### Speed

CPU time of the whole process, best of 5 runs. Both languages run the same plain algorithm;
Python is used without NumPy.

| Benchmark | Kilat (s) | CPython (s) | Times faster | Without start-up |
|---|---|---|---|---|
| collatz | 0.153 | 0.514 | 3.35 | 3.18 |
| fib | 0.059 | 0.197 | 3.33 | 2.88 |
| loop_sum | 0.092 | 0.361 | 3.92 | 3.65 |
| mandelbrot | 0.111 | 0.454 | 4.08 | 3.86 |
| map_count | 0.136 | 0.184 | 1.35 | 1.12 |
| matmul | 0.061 | 0.177 | 2.89 | 2.43 |
| nbody | 0.130 | 0.580 | 4.46 | 4.29 |
| sieve | 0.106 | 0.267 | 2.51 | 2.24 |
| sort | 0.075 | 0.590 | 7.87 | 7.69 |
| strings | 0.130 | 0.178 | 1.37 | 1.12 |

Geometric mean: 3.11 times faster for the whole process and 2.79 with start-up taken out.
Start-up costs Kilat 3 ms of CPU and CPython 35 ms.
With start-up taken out, Kilat is slower than CPython on: none; about level (within 5 %) on: none.
Text and map work is where the two are closest, because CPython's text and dictionary code is very good.
Kilat is a bytecode interpreter, so compiled and JIT languages are faster than both, and Python with
NumPy would win the array benchmarks. The goal set for this version was 5 times CPython; it was not reached.

### Size and dependencies

| Piece | Bytes |
|---|---|
| kilat program | 1,271,808 |
| kilat library | 1,719,296 |
| docs/kilat.wasm | 722,723 |
| docs/kilat.js | 16,007 |
| main.js (Cairn plugin) | 362,037 |

The program and the library need only files that are part of Windows. The WebAssembly engine has 0 imports, so it cannot reach anything the page does not hand it.

### Checks behind these numbers

- Language tests: 6 of 6.
- The Python library prints the same as the program: 11 of 11.
- Sandbox (fuel limit, memory limit, files refused without a flag): 6 of 6.
- In a headless browser: engine checks 79 of 79, playground and Cairn plugin checks 44 of 44.
- In that run the plugin showed its first result 65 ms after the block was drawn, and the longest stall of a page during a run was 101 ms.
- Fuzz and differential tests (`python tests/fuzz_test.py`, seed 20261112): 11,607 generated programs
  and expressions, 0 failures. No crash, no hang, and `check` agreed with `run` every time.
  - 6,061 broken or random programs and 46 deep or runaway ones all ended as an ordinary result or error.
  - 4,000 random number expressions gave the same answer as a Python reference; 299 of them overflow
    or divide by zero, and Kilat refused each of those at run time.
  - 600 `mean` / `median` / `stddev` cases agreed with Python's `statistics`, 600 sums of units agreed,
    and 300 sums of different dimensions were all refused before running.
  - The test and its reference were written by the author of Kilat. Before the run, the harness was
    shown to catch a planted crash and a planted hang.
- Fuel stop location (`python tests/fuel_location_test.py`): 4 of 4.
- The release download check was last run for 0.2.1: `kilat.exe` fetched from that release page had
  the same SHA-256 as the build it came from, the zip held the same program, and the five examples
  printed the same lines.
- Every push reruns `judge.py --quick` and the fuzz test on GitHub's Windows and Linux machines
  (`.github/workflows/ci.yml`). For 0.2.1 both passed: 20 of 20 bugs refused, 11,607 fuzz cases
  with 0 failures, speed against CPython 3.13 of 2.82 (2.76 net of start-up) on Windows and
  2.85 (2.80 net) on Linux, from 2 runs each. Same script, same author; not the headline numbers.

## New in 0.2

- Faster maps, text splitting and small-object allocation.
- `mean`, `median` and `stddev` (sample form) for lists of numbers. They keep the unit:

```kilat
let gaps = [1.9 mm, 2.1 mm, 2.0 mm, 2.4 mm]
print("mean {mean(gaps):mm.2}, median {median(gaps):mm.2}, spread {stddev(gaps):mm.3}")
```

```text
mean 2.10 mm, median 2.05 mm, spread 0.216 mm
```

- The web engine can run in a Web Worker (`{ worker: true }`).
- 0.2.1: a fuel stop now names the line that was running. In 0.2.0 it could name a library
  function instead (`at <prelude>:309 in to_celsius` for an empty `while true { }` loop).
  The fuzz test found this; `python tests/fuel_location_test.py` now checks four cases.
- GlowCode, a Glow page where a person or a model writes Kilat and the checker refuses a wrong
  program before it runs. It is merged into Glow and live at
  https://juw12.github.io/projects/glow/code/ (https://github.com/juw12/projects/pull/53).
  The page still ships the 0.2.0 web engine, so the fuel stop fix above is not in it yet.

## Where Kilat is behind

- No packages and no library ecosystem. No debugger, editor support or interactive prompt.
- No classes, traits, async or threads.
- New and unproven: version 0.2, one author. Measured on one Windows PC; the checks are also
  rerun on GitHub's Windows and Linux machines on every push.
- Fuel counts work (loop turns, calls, the size of long text and list operations), not seconds.
  The time to reach a fuel limit varies, so an app that needs a time limit must add a clock.
- The web engine runs on the page's main thread by default and gives the page a turn every 40 ms.
  Worker mode keeps the page smooth and stops at once, but the same program took 1.5 to 2.5 times
  longer in headless Edge. That figure comes from a separate probe page run by hand, not from `judge.py`.
- The Cairn plugin was tested in Cairn's real web app with its demo brain in headless Edge: 8 of 8 checks, passed twice in a row (`python tests/cairn_app_test.py <cairn checkout>`). A note shows a kilat block's result in Split and Preview modes, not in Write mode. The catalogue entry is merged (https://github.com/juw12/cairn/pull/35) and lists version 0.2.0. Cairn's in-app install from GitHub and Cairn's desktop build were not tested.
  It needs `DecompressionStream`. It keeps the result
  of each program text, so `rand()` and `clock()` stay the same until the blocks are run again.
  A block is stopped after 10 seconds.
- `install.cmd` was run with `/dry-run` from the unpacked release zip; a real install was not run. The release files are unsigned, so Windows may show a SmartScreen prompt.
- The native program uses its own allocator for small blocks. Memory for small blocks is kept at its
  highest level until the program ends, and the command-line program does not free memory on exit.
  `unsafe` Rust is confined to two files, `src/alloc.rs` and `src/ffi.rs`.
- The memory limit is an estimate. It does not count the saved hash of each text.
- The bug set, the fuzz test and its reference were all written by the author. Nobody else has written
  a bug set for Kilat. The measurements were repeated on GitHub's machines by the same script,
  not by another person.
- GlowCode was tested in headless Edge with a stand-in for the model: 17 of 17 page actions in two runs.
  It was not tried with a real model, at phone width, or offline.
- The limits of the language itself are listed at the end of the [language guide](docs/LANGUAGE.md).

## Use it

### Command line (Windows)

```text
kilat run examples\hello.kilat
kilat check examples\bolt_joint.kilat
kilat eval "print(90 km/h in mph)"      prints 55.9234073014
```

Files can only be read with `--allow-read` and written with `--allow-write`.

### Web page

Copy `docs/kilat.js` and `docs/kilat.wasm` next to your page.

```html
<script type="module">
  import { loadKilat } from "./kilat.js";

  const kilat = await loadKilat();
  const result = await kilat.run('print("stress =", 250 N / (4 mm * 3 mm) in MPa)');
  console.log(result.ok, result.output);
</script>
```

`loadKilat(undefined, { worker: true })` runs the engine in a Web Worker instead. The result has
`inWorker` set to `true`, the page stays responsive during long runs, and an abort signal stops a run
at once. It is opt-in because it measured slower (see above).

### Python

Copy `bindings/python/kilat.py` and the built library `kilat_core.dll` next to your script.

```python
import kilat

result = kilat.run('print("stress =", 250 N / (4 mm * 3 mm) in MPa)')
print(result.ok, result.output.strip())

bad = kilat.check("let gap: mm = 2 s")
print(bad.ok, bad.error.splitlines()[0])
```

```text
True stress = 20.8333333333
False error: unit mismatch in the value: expected m, found s
```

### Cairn

Open Plugins, choose "Install from GitHub or a link" and type `juw12/kilat`. A code block marked
`kilat` in a note then shows its result under the code.

### An AI app

Let the model write Kilat, run it in the WebAssembly engine with a fuel and memory limit, and send
any type or unit error back to the model. The program can only reach the outside through one
function, `host(name, payload)`, which the app supplies. That is where an app adds `http_get`,
`ask_ai` or anything else it wants to allow.

## Build

```text
python build.py            native program, WebAssembly engine and web files
python build.py native     target/release/kilat.exe and kilat_core.dll
python build.py wasm       docs/kilat.wasm
python build.py web        docs/examples.js and main.js (the Cairn plugin)
python judge.py            measure everything again and rewrite RESULTS.md
```

- Needs Rust and, for the WebAssembly engine, `rustup target add wasm32-unknown-unknown`.
  The Rust code has no dependencies. `build.py` and `judge.py` use only the Python standard library.
- On Windows without the Microsoft linker, `build.py` uses the GNU toolchain
  (`rustup toolchain install stable-x86_64-pc-windows-gnu`). Set `KILAT_TOOLCHAIN` to choose another.
- `judge.py` uses mypy and Edge or Chrome when they are installed and skips those parts when they are not.

## Licence

MIT. See [LICENSE](LICENSE).
