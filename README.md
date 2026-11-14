# Kilat

Kilat is a small programming language for engineering calculations and safe scripts.
Units and types are checked before a program runs, so `2 m + 3 kg` is refused instead of
printing a number. The whole language is one program of 1.4 MB with no dependencies.
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
`python judge.py` writes. Measured on 2026-11-14 with kilat 0.2.3 and CPython 3.13.2
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
| collatz | 0.224 | 0.649 | 2.90 | 2.64 |
| fib | 0.088 | 0.244 | 2.78 | 2.08 |
| loop_sum | 0.119 | 0.440 | 3.71 | 3.24 |
| mandelbrot | 0.154 | 0.539 | 3.51 | 3.15 |
| map_count | 0.117 | 0.229 | 1.95 | 1.39 |
| matmul | 0.081 | 0.271 | 3.34 | 2.62 |
| nbody | 0.160 | 0.483 | 3.01 | 2.65 |
| sieve | 0.132 | 0.221 | 1.67 | 1.16 |
| sort | 0.056 | 0.554 | 9.93 | 9.61 |
| strings | 0.106 | 0.226 | 2.13 | 1.51 |

Geometric mean: 3.06 times faster for the whole process and 2.49 with start-up taken out.
Start-up costs Kilat 6 ms of CPU and CPython 74 ms.
With start-up taken out, Kilat is slower than CPython on: none; about level (within 5 %) on: none.
The PC was busy with other work during this run: its processors were 62 to 100 % in use in the
9 samples taken while it ran. That makes these figures noisier than a quiet run would give;
CPython's start-up measured 74 ms here against 35 ms in the quiet run for 0.2.1.
The Kilat program that was measured is the file GitHub's Windows machine built for the 0.2.3 release.
This was the third attempt to run `judge.py` for 0.2.3. The first stopped when a four-line Python
helper did not finish within 60 seconds, and it saved no figures. The second finished its timing
(3.09 for the whole process and 2.59 with start-up taken out, from its log; the figures per program
were not saved) and then stopped the same way; the PC had 1.2 GB of memory free when it started.
`judge.py` now keeps the timing as soon as it is measured, gives a check that stalls up to 3 tries
and records each stall, and gives these helpers 300 seconds.
This run recorded 2 stalls, both of the Python-library call for `tests/builtins_edge.kilat` (limit
120 seconds each time); the third try passed. Less than two minutes after the run ended, the same
call took 0.14 to 0.16 s, three times in a row.
No start was repeated to get a better number.
GitHub's machines measured the commit the file for the release was built from twice, each time with
their own build and 2 runs per program: 3.09 and 2.84 for Windows (3.02 and 2.76 with start-up taken out),
and 2.96 and 3.13 for Linux (2.91 and 3.06).
Closest to CPython in this run, with start-up taken out: `sieve` (1.16), `map_count` (1.39) and
`strings` (1.51). The low `sieve` figure is not a slowdown in 0.2.3: timed in turn on this PC, the
0.2.2 and 0.2.3 programs both took 0.137 s (best of 9 rounds). Against the 0.2.2 run, CPython's time
for `sieve` fell from 0.313 s to 0.221 s while Kilat's went from 0.123 s to 0.132 s; the reason was
not found. GitHub's four runs gave 2.06 to 2.43 for `sieve`, and `strings` was the closest program
in each of them (1.16 to 1.41): CPython's text code is very good.
Kilat is a bytecode interpreter, so compiled and JIT languages are faster than both, and Python with
NumPy would win the array benchmarks. The goal set for this version was 5 times CPython; it was not reached.

### Size and dependencies

| Piece | Bytes |
|---|---|
| kilat program | 1,362,944 |
| kilat library | 1,810,944 |
| docs/kilat.wasm | 786,285 |
| docs/kilat.js | 16,007 |
| main.js (Cairn plugin) | 392,381 |

The program and the library need only files that are part of Windows. The WebAssembly engine has 0 imports, so it cannot reach anything the page does not hand it.

The Cairn plugin has to stay under Cairn's limit of 400,000 characters. It is at 392,381, so
7,619 are left; every push rebuilds it on GitHub's machines and fails if it goes over.
In this table the program is the file built for the 0.2.3 release, and the library was built on this PC
from the same source with a GNU toolchain.

### Checks behind these numbers

- Language tests: 7 of 7.
- The Python library prints the same as the program: 12 of 12.
- Sandbox (fuel limit, memory limit, files refused without a flag): 6 of 6.
- In a headless browser: engine checks 81 of 81, playground and Cairn plugin checks 44 of 44.
- In that run the plugin showed its first result 59 ms after the block was drawn, and the longest stall of a page during a run was 143 ms.
- Fuzz and differential tests (`python tests/fuzz_test.py`, seed 20261112): 11,608 generated programs
  and expressions, 0 failures, none skipped. No crash, no hang, and `check` agreed with `run` every time.
  - 6,062 broken or random programs and 46 deep or runaway ones all ended as an ordinary result or error.
  - 4,000 random number expressions gave the same answer as a Python reference; 308 of them overflow
    or divide by zero, and Kilat refused each of those at run time.
  - 600 `mean` / `median` / `stddev` cases agreed with Python's `statistics`, 600 sums of units agreed,
    and 300 sums of, or conversions between, different dimensions were all refused before running,
    each with a unit-mismatch message.
  - The first 0.2.3 runs of this test skipped all 900 unit cases and still reported 0 failures: the
    test asked `print(1 mm)` to see whether a unit exists, and 0.2.3 refuses that line. The question
    is now `print((1 mm) in mm)`, and a unit the test cannot use is a failure, not a skip.
  - The test and its reference were written by the author of Kilat. Before the run, the harness was
    shown to catch a planted crash and a planted hang.
- Fuel stop location (`python tests/fuel_location_test.py`): 4 of 4.
- How values with a unit are shown, and the hints Kilat gives
  (`python tests/unit_hints_test.py`): 75 of 75.
- The release download check was last run for 0.2.2: `kilat.exe` fetched from that release page had
  the same SHA-256 as the build it came from, the zip held the same program, and the five examples
  printed the same lines.
- Every push reruns `judge.py --quick` and the fuzz test on GitHub's Windows and Linux machines
  (`.github/workflows/ci.yml`). The commit the file for the 0.2.3 release was built from was run
  twice there and passed both times: 20 of 20 bugs refused, speed against CPython 3.13 of 3.09 and 2.84
  (3.02 and 2.76 net of start-up) for Windows and 2.96 and 3.13 (2.91 and 3.06 net) for Linux, from 2 runs
  per program. The Windows machine there builds with Microsoft's toolchain; the file for the
  release was built with the GNU one. The fuzz test in those runs is the one that skipped the
  900 unit cases. Same script, same author; not the headline numbers.

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
- 0.2.2: three checker messages for mistakes that free models made in GlowCode
  (`python tests/unit_hints_test.py`):
  - `speed * 2 * pi / 60` when `speed` was written with `rpm` is refused. A speed written with
    `rpm` is already stored as an angular speed, so that line would convert it a second time
    and print a wrong torque.
  - `"{force} N"` is refused when `{force}` already prints its own unit. Write `{force:N}`.
  - Showing a plain number with a unit (`{energy:J}` after `let energy = work in J`) now says
    why: `x in J` gives a plain number. Keep the unit on the value, or write the unit after
    the braces.
- 0.2.3: a value with a unit is never shown without a named unit. `print(d)`, `"{d}"` and `str(d)`
  are refused before the program runs, and the message suggests the unit the program itself used
  (``write `{d:mm}` ``). Before, `2.4 mm` printed as `0.0024 m`, `1450 rpm` as
  `151.843644924 1/s`, and a unit inside a list, a struct or a generic function was dropped. The
  same rule covers the last line of `kilat eval`, a Cairn block and the playground.
  - This breaks programs that printed a bare value with a unit: 12 of the 61 Kilat program files in
    this repository had to be changed (ten are the two halves of 5 bug pairs).
    `python tests/unit_hints_test.py` now has 75 cases; 0.2.2 passes 40 of them.
  - A failed `assert_eq` on two values that print the same now says so and points to
    `assert(approx(a, b, tolerance))`.
  - `get_or` on maps is faster, and so are `has`, `starts_with`, `ends_with` and `approx` in the
    small programs written to time them. Measured before and after on GitHub's machines, two
    samples per system with 9 rounds each (`python tests/compare_builds.py`,
    `.github/workflows/compare.yml`, programs in `compare/`): for Windows `map_count` ran 1.68 and
    1.59 times faster and the ten benchmarks together 1.09 and 1.08, with nothing slower. For Linux
    `map_count` ran 1.57 and 1.51 times faster and the ten together 1.05 and 1.05, but `nbody` ran at
    0.84 and 0.94 times its earlier speed, slower in both samples, and the cause was not found;
    3 other Linux programs were slower in one sample only.
    More than half of the gain on the ten benchmarks is `map_count`: the other nine together ran at
    1.04 and 1.03 times their earlier speed for Windows and 1.00 and 1.01 for Linux. `abs`, `int`, `trunc`
    and `min` of two numbers showed no clear change in any sample, nor did `strings`, a longer text
    program that also calls `ends_with` (it is timed twice: the same file is in the small set as
    `strings_full`). `floor`, `ceil`, `round` and `min` of two texts were judged faster in some
    samples only, and `is_nan` faster in three and slower in one; details are in `RESULTS.md`.
    The outputs of 193 runs, 23 error messages and the fuel use of 56 programs did not change.
    The web engine's speed was not measured.
- GlowCode, a Glow page where a person or a model writes Kilat and the checker refuses a wrong
  program before it runs. It is merged into Glow and live at
  https://juw12.github.io/projects/glow/code/ (https://github.com/juw12/projects/pull/53).
  The page ships the 0.2.2 web engine (https://github.com/juw12/projects/pull/55).

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
- The Cairn plugin was tested in Cairn's real web app with its demo brain in headless Edge: 8 of 8 checks (`python tests/cairn_app_test.py <cairn checkout>`; run once with the 0.2.3 plugin, twice in a row with 0.2.2). A note shows a kilat block's result in Split and Preview modes, not in Write mode. The catalogue entry is merged (https://github.com/juw12/cairn/pull/35) and lists version 0.2.2 (https://github.com/juw12/cairn/pull/40). Cairn's in-app install from GitHub and Cairn's desktop build were not tested.
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
  With the page's real free models, 10 engineering questions asked 3 times each gave 27 of 30 correct
  on 0.2.2 (24 of 30 on 0.2.1), and 4 new questions gave 4 of 4. Many of the 30 answers were replayed
  by the provider and the model prompt contains one of the test numbers, so this is an indication,
  not a proof; see `RESULTS.md`. In that sample a value printed with no unit (`{largest}`) showed in
  base SI (`0.0024 m`, not `2.4 mm`); 0.2.3 refuses a program of that kind before it runs (checked
  with a program written for the purpose, not the model's own), but the page has not been sampled
  again with 0.2.3. The page was not tried at phone width or offline.
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
