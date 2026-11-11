# Kilat

Kilat is a small programming language for engineering calculations and safe scripts.
Units and types are checked before a program runs, so `2 m + 3 kg` is refused instead of
printing a number. The whole language is one program of 1.1 MB with no dependencies.
The same engine also runs in a web page, inside Python, and as a plugin for the Cairn notes app.
"Kilat" is the Malay word for lightning.

- Try it in the browser: <https://juw12.github.io/kilat/>
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
`python judge.py` writes. Measured on 2026-11-11 with kilat 0.1.0 and CPython 3.13.2
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
| collatz | 0.345 | 0.775 | 2.25 | 2.09 |
| fib | 0.087 | 0.307 | 3.53 | 2.98 |
| loop_sum | 0.121 | 0.540 | 4.45 | 4.11 |
| mandelbrot | 0.143 | 0.657 | 4.60 | 4.32 |
| map_count | 0.278 | 0.284 | 1.02 | 0.80 |
| matmul | 0.085 | 0.247 | 2.92 | 2.30 |
| nbody | 0.145 | 0.593 | 4.10 | 3.80 |
| sieve | 0.146 | 0.288 | 1.98 | 1.58 |
| sort | 0.091 | 0.761 | 8.37 | 8.21 |
| strings | 0.228 | 0.225 | 0.99 | 0.71 |

Geometric mean: 2.82 times faster for the whole process and 2.43 with start-up taken out.
Start-up costs Kilat 7 ms of CPU and CPython 68 ms.
With start-up taken out, Kilat is slower than CPython on: `map_count`, `strings`. CPython keeps the hash of a text
once it is computed and has a faster allocator for small objects. Kilat is a bytecode interpreter,
so compiled and JIT languages are faster than both, and Python with NumPy would win the array benchmarks.

### Size and dependencies

| Piece | Bytes |
|---|---|
| kilat program | 1,144,832 |
| kilat library | 1,633,792 |
| docs/kilat.wasm | 715,723 |
| docs/kilat.js | 9,490 |
| main.js (Cairn plugin) | 352,472 |

The program and the library need only files that are part of Windows. The WebAssembly engine has 0 imports, so it cannot reach anything the page does not hand it.

### Checks behind these numbers

- Language tests: 5 of 5.
- The Python library prints the same as the program: 10 of 10.
- Sandbox (fuel limit, memory limit, files refused without a flag): 6 of 6.
- In a headless browser: engine checks 37 of 37, playground and Cairn plugin checks 44 of 44.
- In that run the plugin showed its first result 165 ms after the block was drawn, and the longest stall of a page during a run was 233 ms.

## Where Kilat is behind

- No packages and no library ecosystem. No debugger, editor support or interactive prompt.
- No classes, traits, async or threads.
- New and unproven: version 0.1, one author, tested on one Windows PC.
- Fuel counts work (loop turns, calls, the size of long text and list operations), not seconds.
  The time to reach a fuel limit varies, so an app that needs a time limit must add a clock.
- The web engine runs on the page's main thread and gives the page a turn every 40 ms.
- The Cairn plugin was tested against a stand-in for Cairn's plugin loader, not inside the app,
  and it is not in Cairn's plugin catalogue yet. It needs `DecompressionStream`. It keeps the result
  of each program text, so `rand()` and `clock()` stay the same until the blocks are run again.
  A block is stopped after 10 seconds.
- `install.cmd` has only been exercised with `/dry-run`.
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
