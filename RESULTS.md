# Measured results

Written by `python judge.py` on 2026-11-14. Kilat kilat 0.2.3, CPython 3.13.2, Windows 10, Intel(R) Core(TM) Ultra 7 258V.

## Speed against CPython

Best of 5 runs each. CPU time is the whole process, measured with `QueryProcessCycleTime`, with both languages kept on the fastest cores.
"Net" takes each language's start-up time off first, which is the stricter figure for Kilat.
Both languages run the same plain algorithm; Python with NumPy would be faster on some of these.
Wall-clock time on the development PC is dominated by how long a process takes to start, so it is shown last.

| Program | Kilat CPU s | CPython CPU s | Times faster | Net of start-up | Kilat wall s | CPython wall s |
|---|---|---|---|---|---|---|
| `collatz` | 0.224 | 0.649 | 2.90 | 2.64 | 0.327 | 1.460 |
| `fib` | 0.088 | 0.244 | 2.78 | 2.08 | 0.180 | 3.390 |
| `loop_sum` | 0.119 | 0.440 | 3.71 | 3.24 | 0.221 | 7.221 |
| `mandelbrot` | 0.154 | 0.539 | 3.51 | 3.15 | 0.206 | 0.628 |
| `map_count` | 0.117 | 0.229 | 1.95 | 1.39 | 0.149 | 8.387 |
| `matmul` | 0.081 | 0.271 | 3.34 | 2.62 | 0.159 | 1.677 |
| `nbody` | 0.160 | 0.483 | 3.01 | 2.65 | 0.374 | 1.103 |
| `sieve` | 0.132 | 0.221 | 1.67 | 1.16 | 0.187 | 6.116 |
| `sort` | 0.056 | 0.554 | 9.93 | 9.61 | 0.109 | 13.333 |
| `strings` | 0.106 | 0.226 | 2.13 | 1.51 | 0.145 | 0.379 |

- Geometric mean: 3.06 times faster for the whole process, 2.49 net of start-up.
- Net of start-up Kilat is faster on 10 of 10 programs.
- The target set before measuring was 5 times: not met.
- The two languages print the same text on every program: yes.
- Start-up: Kilat 6 ms of CPU (15 ms wall), CPython 74 ms (116 ms wall).

## Sizes

| File | Bytes |
|---|---|
| kilat program | 1,362,944 |
| kilat library | 1,810,944 |
| docs/kilat.wasm | 786,285 |
| docs/kilat.js | 16,007 |
| main.js (Cairn plugin) | 392,381 |

The kilat program needs only these libraries: api-ms-win-core-synch-l1-2-0.dll, api-ms-win-crt-environment-l1-1-0.dll, api-ms-win-crt-heap-l1-1-0.dll, api-ms-win-crt-locale-l1-1-0.dll, api-ms-win-crt-math-l1-1-0.dll, api-ms-win-crt-private-l1-1-0.dll, api-ms-win-crt-runtime-l1-1-0.dll, api-ms-win-crt-stdio-l1-1-0.dll, api-ms-win-crt-string-l1-1-0.dll, bcryptprimitives.dll, kernel32.dll, ntdll.dll.

The kilat library needs only these libraries: api-ms-win-core-synch-l1-2-0.dll, bcryptprimitives.dll, kernel32.dll, msvcrt.dll, ntdll.dll, userenv.dll, ws2_32.dll.
All of them are part of Windows: yes.
`docs/kilat.wasm` imports 0 things from its host.

## Mistakes caught before the program runs

20 small programs in `bugs/`, each with one mistake, a fixed version and a Python twin.
The corpus was written by the author of Kilat, so read it as a list of what Kilat checks, not as a survey.

| Kind of mistake | Programs | Kilat refuses | mypy --strict reports | Python refuses before running |
|---|---|---|---|---|
| units | 5 | 5 | 0 | 0 |
| names | 4 | 4 | 4 | 0 |
| types | 6 | 6 | 6 | 0 |
| safety | 5 | 5 | 4 | 0 |
| **all** | 20 | 20 | 14 | 0 |

- Fixed versions that run: 20 of 20.
- Python twins when run: 10 stop with an error, 10 finish with a wrong or meaningless answer.
- Python can check units while a program runs with a library such as pint; that is not measured here.

## Checks

| Check | Passed |
|---|---|
| Conformance programs print their recorded output | 7 of 7 |
| The library prints what the program prints | 12 of 12 |
| Limits and file access on the command line | 6 of 6 |
| Behaviours quoted in the README | 6 of 6 |
| WebAssembly build prints what the program prints | 12 of 12 |
| Engine in a browser | 81 of 81 |
| Playground and Cairn plugin pages | 44 of 44 |

| Behaviour | Kilat | Python |
|---|---|---|
| `round_to(2.675, 2)` | 2.68 | 2.67 |
| `"{1.005:.2}"` | 1.01 | 1.00 |
| `0.0 / 0.0` | runtime error: division by zero | ZeroDivisionError |
| `sqrt(-1.0)` | runtime error: sqrt needs a number that is not negative | ValueError |
| `9223372036854775807 + 1` | runtime error: integer overflow | 9223372036854775808 |
| `a misspelt name` | did you mean `length`? | NameError when the line runs |
| `1e308 * 10.0` | inf | |
| `infinity - infinity` | nan | |

Times seen in the browser checks, in milliseconds unless the name says otherwise:

- `playgroundLimitMs`: 100
- `playgroundLongestFreezeMs`: 101
- `pluginCharacters`: 392381
- `pluginFirstResultMs`: 59
- `pluginLimitMs`: 53
- `pluginLongTextLimitMs`: 2802
- `pluginLongestFreezeMs`: 143
- `pluginReadyMs`: 286
- `pluginRedrawMs`: 16
- `pluginTimeLimitMs`: 10050

Helper programs that did not finish within their time limit and were started again (2): python.exe kilat.py builtins_edge.kilat (limit 120 s); python.exe kilat.py builtins_edge.kilat (limit 120 s).

## About this speed run (0.2.3)

Added after the run by a separate script (not by `judge.py`), so rerunning `judge.py` drops this section
and the ones below. Every figure here is read from a file saved during the work.

- The program measured is the `kilat.exe` of the 0.2.3 release (published 2026-11-16,
  https://github.com/juw12/kilat/releases/tag/v0.2.3): 1,362,944 bytes, SHA-256
  `db646696dc987dff95ebc9b64df5de275e0311cd8170479582aba07d0a0fe9cc`. GitHub's Windows machine built it
  with the GNU toolchain from commit `fe73fa1` (run 43418571070, the comparison of the next section). The
  release is tagged at the later commit `858dc6b`; the source of the program (`src/`, `Cargo.toml` and
  `Cargo.lock`) is the same in both commits.
- GitHub built that commit twice in that run (attempts 1 and 2; the second is a rerun asked for by the
  author to get a second sample). The two programs have the same size and differ in 4 bytes, all inside
  the time-stamp and checksum fields of the file header. The measured file is the first build. GitHub's
  run page now lists only the files of the second build; the first build is the file published in the
  release (see "Release download check").
- Two later runs on GitHub's Windows machines built the same source again (runs 43586788453 and
  43588475930). Each of those two programs has the same size as the released file and differs from it in
  5 bytes, again all inside the time-stamp and checksum fields of the header. On GitHub's Linux machines
  the four builds of that source made by the comparison workflow have the same SHA-256.
- The library (`kilat_core.dll`, the "kilat library" of the size table) was not built by GitHub. The
  check "The library prints what the program prints" loads it. It was built on this PC with a GNU
  toolchain from the same 20 source files: each was compared with commit `fe73fa1`, and none was changed
  after the build started. The version of that toolchain was not recorded. The library links `msvcrt.dll`
  as its C runtime and the GitHub-built program does not, so the two toolchains are not the same.
- The PC was in use for other work during the run. In the 9 load samples taken during the run, 62 to
  100 % of the processors were in use, 5 to 290 threads were waiting for a processor, 10.5 to 16.3 GB of
  the PC's 31 GB of memory were free, and paging reached 2,291 pages per second.
- The wall times in the table are the best of the 5 runs, as the CPU times are (not always from the same
  run). Kilat's wall times were 1.3 to 2.3 times its CPU times and CPython's 1.2 to 37 times; the
  difference is time a process spent waiting, not computing. CPython's start-up measured 74 ms of CPU
  against 35 ms in the quiet run for 0.2.1.
- The "Net of start-up" column subtracts the start-up estimates, so it moves when they move. `strings`
  shows 1.51 here, 0.90 in the 0.2.2 run and 1.12 in the 0.2.1 run.
- `sieve` shows 1.67 (1.16 net) here against 2.55 (2.00 net) in the 0.2.2 run. That is not a slowdown in
  0.2.3: timed in turn on this PC for 9 rounds, the 0.2.2 and the 0.2.3 program took the same time (best
  0.137 s each; medians 0.142 s and 0.143 s). In that check CPython took 0.260 s, so Kilat was 1.90 times
  CPython for the whole process and 1.47 net. Between the 0.2.2 run and this one CPython's `sieve` time
  fell from 0.313 s to 0.221 s while Kilat's went from 0.123 s to 0.132 s. GitHub's eight runs gave 1.73
  to 2.48 net. Why Kilat's time did not fall with CPython's was not found. One guess, not tested: the
  list takes 16 bytes per item in Kilat against 8 in CPython (peak memory of the process minus that of an
  empty program, over 2,000,001 items).
- The judge was started three times for 0.2.3:
  - The first start stopped in the bug table when the Python helper `bugs/name_typo_in_variable.py` (4
    lines) did not finish within 60 seconds. It saved no figures.
  - The second start (2026-11-14 02:21 UTC) measured 3.09 times CPython for the whole process and 2.59
    net of start-up; only that line of its log was kept, not the figures per program. It then stopped
    when `bugs/safety_constant_changed.py` (6 lines) missed the 60-second limit 3 times in a row. In the
    three load samples taken before it stopped, 1.2 to 4.4 GB were free and paging, read in two of them,
    reached 1,608 pages per second. A few minutes after the stop, with 11.6 GB free, the same helper ran
    in 0.38 s.
  - The third start (2026-11-14 03:32 UTC, 50.5 minutes) gave this report. This run recorded 2 stalls,
    both of the Python-library call for `tests/builtins_edge.kilat` (limit 120 seconds each time); the
    third try passed. Less than two minutes after the run ended, the same call took 0.14 to 0.16 s, three
    times in a row.
- After the first start `judge.py` was changed to give a stalled helper up to 3 tries and to record each
  stall. After the second it was changed to keep the timing in a file as soon as it is measured
  (`--reuse-timing` takes it from that file in a later run) and to give the Python helpers of the bug
  table 300 seconds instead of 60. The timing in this report was measured in this run, not reused.
- Earlier versions on this PC: 0.2.2, with the PC also in use for other work, 3.02 times CPython for the
  whole process and 2.54 net of start-up, with `strings` slower than CPython net of start-up; 0.2.1, on a
  quiet PC, 3.11 times and 2.79 net, 10 of 10 faster.
- The 0.2.2 run and this run were made in different states of the PC, so the difference between 3.02 and
  3.06 says little about the change in Kilat. The next section compares builds on the same machine
  instead.
- No start was repeated to get a better number. "Reruns on GitHub's machines" below gives the same
  measurement on other machines.

## Before and after the speed changes (0.2.3)

Added by the same separate script, from the reports of `python tests/compare_builds.py`, which
`.github/workflows/compare.yml` runs on GitHub's machines. Each machine builds two versions of Kilat from
source and times them in turn, 9 rounds per program, in CPU time. "Before" is commit `129aa38` (0.2.3
with the new unit rule, without the speed changes) and "after" is commit `fe73fa1` (the same with the two
speed changes); both call themselves `kilat 0.2.3`. A ratio above 1 means that the "after" build is
faster. The comparison ran twice (attempts 1 and 2 of run 43418571070; the second is a rerun asked for to
get a second sample), so there are two samples per machine, and all four are shown.

The two changes:

- `get_or` on a map, as in `counts.get_or(key, 0)`, became one instruction of the virtual machine.
- 15 builtins are answered on a short path, ahead of the general one: `abs`, `min`, `max`, `floor`,
  `ceil`, `round`, `trunc`, `int`, `float`, `is_nan`, `approx`, `starts_with`, `ends_with`, `get_or` and
  `has`. For `min` and `max` this is the form with two values, not the one that takes a list. For two of
  the 15 a direct call does not go through that path: a call of `get_or` compiles to the new instruction,
  and a call of `float` already compiled to an instruction of its own before these changes. No timed
  program calls `max` or `float`.

| Sample | Processor | 10 benchmarks, best / median | 9 benchmarks without `map_count`, best | 32 small programs, best / median | Same build timed twice, 10 benchmarks, best / median | Faster | No clear change | Slower |
|---|---|---|---|---|---|---|---|---|
| Windows, first | AMD EPYC 9V74 80-Core Processor | 1.09 / 1.08 | 1.04 | 1.10 / 1.10 | 1.00 / 1.00 | 21 | 22 | 0 |
| Windows, second | INTEL(R) XEON(R) PLATINUM 8573C | 1.08 / 1.08 | 1.03 | 1.06 / 1.06 | 1.00 / 1.00 | 16 | 27 | 0 |
| Linux, first | not reported | 1.05 / 1.04 | 1.00 | 1.06 / 1.06 | 1.00 / 1.00 | 14 | 26 | 3 |
| Linux, second | not reported | 1.05 / 1.04 | 1.01 | 1.06 / 1.06 | 1.00 / 1.00 | 18 | 23 | 2 |

The figures are geometric means of the ratios (time before divided by time after), once for the best and
once for the median time of the 9 rounds. "Same build timed twice" checks the measurement itself: every
round also times the "before" build a second time, and the column gives the ratio of those two timings of
one build, which would be 1.00 without noise. The last three columns count the verdicts over all 43 timed
programs: the 10 benchmarks and 33 small programs in `compare/micro/`. One of the small programs is empty
and only measures start-up; it has a verdict but is left out of the mean. Another, `micro/strings_full`,
is the same file as the benchmark `bench/strings`: that program is timed twice, counts in both means and
has two verdicts, so the 43 timed programs are 42 different ones.

More than half of the gain on the 10 benchmarks comes from one of them, `map_count` (1.68, 1.59, 1.57 and
1.51 in the four samples). Without it the other 9 give 1.04 and 1.03 on Windows and 1.00 and 1.01 on
Linux.

18 of the 43 timed programs call at least one of the 15 builtins (17 different programs, because
`bench/strings` and `micro/strings_full` are the one file that is timed twice). All 18 are listed, with
the ratio of best times and the verdict in each sample:

| Program | Calls | Windows, first | Windows, second | Linux, first | Linux, second |
|---|---|---|---|---|---|
| `bench/map_count` | `get_or` | 1.68 faster | 1.59 faster | 1.57 faster | 1.51 faster |
| `bench/strings` | `ends_with` | 1.03 no clear change | 1.02 no clear change | 1.00 no clear change | 1.01 no clear change |
| `micro/call_b_abs` | `abs` | 1.01 no clear change | 0.99 no clear change | 1.03 no clear change | 0.99 no clear change |
| `micro/call_c_min` | `min` | 1.00 no clear change | 0.99 no clear change | 1.00 no clear change | 1.02 no clear change |
| `micro/call_f_floor` | `floor` | 1.04 faster | 1.02 no clear change | 1.05 faster | 1.06 faster |
| `micro/call_j_ceil` | `ceil` | 1.05 faster | 1.01 no clear change | 1.00 no clear change | 1.04 faster |
| `micro/call_k_round` | `round` | 1.05 faster | 1.03 no clear change | 1.03 no clear change | 1.05 faster |
| `micro/call_l_trunc` | `trunc` | 1.00 no clear change | 0.99 no clear change | 1.00 no clear change | 1.01 no clear change |
| `micro/call_m_int` | `int` | 1.00 no clear change | 0.98 no clear change | 1.00 no clear change | 1.01 no clear change |
| `micro/call_n_is_nan` | `is_nan` | 1.04 faster | 1.04 faster | 0.97 slower | 1.08 faster |
| `micro/call_o_approx` | `approx` | 1.13 faster | 1.29 faster | 1.03 faster | 1.09 faster |
| `micro/map_b_get_or` | `get_or` | 1.93 faster | 1.62 faster | 1.73 faster | 1.59 faster |
| `micro/map_e_has` | `has` | 1.19 faster | 1.12 faster | 1.12 faster | 1.13 faster |
| `micro/map_f_get_or_computed` | `get_or` | 1.78 faster | 1.46 faster | 1.51 faster | 1.47 faster |
| `micro/strings_full` | `ends_with` | 1.03 no clear change | 1.02 no clear change | 1.01 no clear change | 1.00 no clear change |
| `micro/text_a_ends_with` | `ends_with` | 1.25 faster | 1.13 faster | 1.11 faster | 1.08 faster |
| `micro/text_b_starts_with` | `starts_with` | 1.19 faster | 1.15 faster | 1.09 faster | 1.16 faster |
| `micro/text_c_min` | `min` | 1.08 faster | 1.06 faster | 1.01 no clear change | 0.97 no clear change |

Of these 18 programs:

- 7 were judged faster in all four samples: `bench/map_count`, `micro/call_o_approx`,
  `micro/map_b_get_or`, `micro/map_e_has`, `micro/map_f_get_or_computed`, `micro/text_a_ends_with` and
  `micro/text_b_starts_with`.
- 5 were judged faster in some samples but not in all: `micro/call_f_floor`, `micro/call_j_ceil`,
  `micro/call_k_round`, `micro/call_n_is_nan` and `micro/text_c_min` (`micro/call_n_is_nan` was judged
  slower in the first Linux sample).
- 6 were not judged faster in any sample (no clear change every time): `bench/strings`,
  `micro/call_b_abs`, `micro/call_c_min`, `micro/call_l_trunc`, `micro/call_m_int` and
  `micro/strings_full`. Two of these are the one file that is timed twice, so they are 5 different
  programs. No gain was seen for `abs`, `int` and `trunc`: no program that calls one of them was judged
  faster.

The verdicts of those 18 programs against the other 25:

| Sample | Callers: faster | Callers: no clear change | Callers: slower | Others: faster | Others: no clear change | Others: slower |
|---|---|---|---|---|---|---|
| Windows, first | 12 | 6 | 0 | 9 | 16 | 0 |
| Windows, second | 9 | 9 | 0 | 7 | 18 | 0 |
| Linux, first | 8 | 9 | 1 | 6 | 17 | 2 |
| Linux, second | 11 | 7 | 0 | 7 | 16 | 2 |

So programs that call none of the 15 builtins were judged faster too: 9, 7, 6 and 7 of the 25 in the four
samples. None of them reaches one of these builtins through Kilat's own library functions either: four
functions in `src/prelude.kilat` do (`as_int`, `get`, `group_by` and `zip`), and no timed program uses
any of them. Why these programs moved was not looked into. One fact that may bear on it, not tested: the
second change moved the general path of all other builtins into a function of its own (`call_rest` in
`src/natives.rs`).

Slower verdicts appear on Linux only. `bench/nbody` was judged slower in both Linux samples, at 0.84 and
0.94 times its earlier speed; on Windows it measured 1.05 (faster) and 1.02 (no clear change). The cause
was not found. Other programs were judged slower in one Linux sample only: in the first,
`bench/mandelbrot` (0.96) and `micro/call_n_is_nan` (0.97); in the second, `bench/sort` (0.97). No
program was judged slower in either Windows sample.

Both builds printed the same in 193 runs, gave the same 23 error messages and used the same fuel in 56
programs, in all four reports.

The rule for the verdicts is part of the tool and was committed with it (`129aa38`) before these
comparisons ran. For each program the band is 3 %, or twice the relative difference between the two
timings of the "before" build if that is larger (taking the larger of the differences in best and in
median time). A program is "faster" only when the ratio of best times and the ratio of median times are
both above 1 + band, "slower" only when both are below 1 / (1 + band), and "no clear change" otherwise.

For scale: the same tool also compared the change before this one, the new unit rule (commits `94690f4`
to `129aa38`, run 43418571031, one sample per machine), which was not meant to change speed. It judged 4
faster and 1 slower on Windows and 0 faster and 1 slower on Linux of 43 programs, with geometric means
from 0.99 to 1.02. So a single verdict in the tables above can be noise; the same verdict in all four
samples is stronger.

Both changes were kept. Each of the two changes was later measured on its own, and so was the whole step
from 0.2.2 to 0.2.3. The two parts below report that; they were added on 2026-11-16 by a further script,
which is not in the repository, from the reports of the runs they name. Not measured: the speed of the
web engine (`docs/kilat.wasm`).

### The whole step from 0.2.2 to 0.2.3

Run 43586788453 of the same workflow compared commit `94690f4` (`kilat 0.2.2`: `main` before this work,
with the same program source as the 0.2.2 release) with commit `858dc6b` (`kilat 0.2.3`, the commit of
the release tag). There is one sample per machine, with 9 rounds per program. The step covers the new
unit rule and both speed changes together.

| Sample | Processor | 10 benchmarks, best / median | 9 benchmarks without `map_count`, best | 32 small programs, best / median | Same build timed twice, 10 benchmarks, best / median | Faster | No clear change | Slower |
|---|---|---|---|---|---|---|---|---|
| Windows | AMD EPYC 9V45 96-Core Processor | 1.06 / 1.06 | 1.02 | 1.06 / 1.06 | 1.00 / 1.00 | 19 | 24 | 0 |
| Linux | not reported | 1.06 / 1.04 | 1.01 | 1.06 / 1.07 | 0.99 / 1.00 | 17 | 24 | 2 |

Each benchmark, with the ratios of best and of median times and the verdict:

| Program | Windows | Linux |
|---|---|---|
| `bench/collatz` | 0.98 / 1.02 no clear change | 1.29 / 1.10 faster |
| `bench/fib` | 1.00 / 0.97 no clear change | 1.06 / 1.08 faster |
| `bench/loop_sum` | 1.04 / 1.03 faster | 0.93 / 0.94 slower |
| `bench/mandelbrot` | 1.11 / 1.12 faster | 1.04 / 0.99 no clear change |
| `bench/map_count` | 1.45 / 1.49 faster | 1.59 / 1.58 faster |
| `bench/matmul` | 1.07 / 1.04 no clear change | 1.00 / 0.98 no clear change |
| `bench/nbody` | 1.02 / 0.99 no clear change | 0.84 / 0.82 slower |
| `bench/sieve` | 0.99 / 0.99 no clear change | 0.99 / 0.98 no clear change |
| `bench/sort` | 1.01 / 1.00 no clear change | 1.01 / 1.02 no clear change |
| `bench/strings` | 1.00 / 1.01 no clear change | 1.01 / 1.00 no clear change |

- `bench/map_count` ran 1.45 times as fast on Windows and 1.59 on Linux (ratios of best times). Without
  it the other 9 benchmarks give 1.02 and 1.01.
- No program was judged slower on Windows. On Linux two were: `bench/nbody` (0.84 best, 0.82 median) and
  `bench/loop_sum` (0.93 best, 0.94 median). The next part looks at both.
- Of the 193 runs whose output was compared, 2 differ, the same 2 in both reports: `tests/units.kilat`,
  once run and once only checked. That file was changed for 0.2.3 and now uses the format `{4 Hz:1/s}`,
  which 0.2.2 refuses (its message: "cannot read the format `1/s`") and 0.2.3 accepts.
- The 23 error messages were the same in both builds. The fuel use of 55 programs was the same;
  `tests/units` was left out of that comparison because it does not end the same way in both builds.

### Each speed change on its own

Runs 43588473616 and 43588475930 of the same workflow, started by hand, each compared two neighbouring
commits: `0e17f86` with `83eefbb`, which adds only the `get_or` instruction, and `83eefbb` with
`705bc2f`, which adds only the short path. There is one sample per machine, with 9 rounds per program;
all four builds call themselves `kilat 0.2.3`. The program source of `705bc2f` is the same as in the
release.

| Sample | Processor | 10 benchmarks, best / median | 9 benchmarks without `map_count`, best | 32 small programs, best / median | Same build timed twice, 10 benchmarks, best / median | Faster | No clear change | Slower |
|---|---|---|---|---|---|---|---|---|
| `get_or` only, Windows | AMD EPYC 7763 64-Core Processor | 1.05 / 1.04 | 1.01 | 1.02 / 1.02 | 0.99 / 0.99 | 6 | 33 | 4 |
| `get_or` only, Linux | not reported | 1.01 / 1.01 | 0.95 | 1.03 / 1.03 | 1.01 / 1.00 | 3 | 38 | 2 |
| Short path only, Windows | AMD EPYC 9V45 96-Core Processor | 1.03 / 1.03 | 1.04 | 1.03 / 1.03 | 1.00 / 1.00 | 17 | 25 | 1 |
| Short path only, Linux | not reported | 1.04 / 1.04 | 1.05 | 1.04 / 1.05 | 1.03 / 1.00 | 12 | 29 | 2 |

Each benchmark, with the ratios of best and of median times and the verdict:

| Program | `get_or` only, Windows | `get_or` only, Linux | Short path only, Windows | Short path only, Linux |
|---|---|---|---|---|
| `bench/collatz` | 1.23 / 1.10 faster | 0.97 / 0.98 no clear change | 1.03 / 1.03 no clear change | 1.13 / 1.13 no clear change |
| `bench/fib` | 1.02 / 1.01 no clear change | 1.01 / 1.03 no clear change | 1.01 / 1.01 no clear change | 1.07 / 1.09 faster |
| `bench/loop_sum` | 0.92 / 0.95 slower | 0.66 / 0.68 slower | 1.02 / 1.02 no clear change | 1.47 / 1.48 faster |
| `bench/mandelbrot` | 0.92 / 0.92 slower | 0.96 / 0.96 no clear change | 1.11 / 1.11 faster | 0.97 / 0.99 no clear change |
| `bench/map_count` | 1.57 / 1.60 faster | 1.79 / 1.76 faster | 0.99 / 0.99 no clear change | 1.01 / 1.01 no clear change |
| `bench/matmul` | 1.00 / 1.00 no clear change | 1.00 / 1.00 no clear change | 1.04 / 1.04 faster | 1.02 / 1.00 no clear change |
| `bench/nbody` | 0.99 / 0.98 no clear change | 1.00 / 0.99 no clear change | 1.03 / 1.03 no clear change | 0.85 / 0.84 slower |
| `bench/sieve` | 0.99 / 0.99 no clear change | 1.01 / 1.00 no clear change | 1.12 / 1.12 faster | 1.00 / 0.99 no clear change |
| `bench/sort` | 0.99 / 0.98 no clear change | 1.01 / 1.00 no clear change | 1.00 / 1.00 no clear change | 1.00 / 1.01 no clear change |
| `bench/strings` | 1.01 / 1.01 no clear change | 0.99 / 0.99 no clear change | 1.00 / 1.01 no clear change | 1.01 / 0.99 no clear change |

Every program judged slower, with the ratios of best and of median times:

- `get_or` only, Windows: `bench/loop_sum` 0.92 / 0.95, `bench/mandelbrot` 0.92 / 0.92,
  `micro/call_g_float_only` 0.93 / 0.84, `micro/call_h_sqrt` 0.88 / 0.86.
- `get_or` only, Linux: `bench/loop_sum` 0.66 / 0.68, `micro/call_d_len` 0.94 / 0.94.
- Short path only, Windows: `micro/fill_1m` 0.95 / 0.95.
- Short path only, Linux: `bench/nbody` 0.85 / 0.84, `micro/call_n_is_nan` 0.96 / 0.97.

What the two runs show:

- The `bench/map_count` gain comes from the `get_or` instruction. With that change alone the program ran
  1.57 times as fast on Windows and 1.79 on Linux (ratios of best times, judged faster on both). With the
  short path alone it gave 0.99 and 1.01, no clear change on both.
- In the `get_or`-only sample on Linux the 9 benchmarks without `bench/map_count` come to 0.95. One
  program accounts for that, `bench/loop_sum` at 0.66: the remaining 8 give 0.99.
- On Linux `bench/nbody` ran slower with the short path alone: 0.85 best, 0.84 median. The other three
  samples of these two runs show no clear change for it. Counting the earlier runs, it was judged slower
  in each of the 4 Linux comparisons whose "after" build has the short path and whose "before" build does
  not (0.82 to 0.94 over their ratios of best and of median times). The "after" program of those 4 is one
  and the same file, byte for byte, so they are 4 timings of one build against 3 different "before"
  builds, not 4 independent builds.
- `bench/nbody` calls none of the 15 builtins of the short path. It calls `sqrt` and `len`, which compile
  to instructions of their own and do not go through the function that the commit changed, and `print`,
  twice, outside its loops. What the commit changed in structure: it split the function that runs a
  builtin in two, a small one that answers the 15 builtins and a second one, kept out of line, for all
  the others. Every other builtin that has no instruction of its own is now reached through the small
  function, which hands the call on. Whether that, or the place the compiler gave the code of the virtual
  machine in the new build, is behind the Linux result was not tested. The cause was not found.
- On Linux `bench/loop_sum` was judged slower with the `get_or` change alone (0.66 best, 0.68 median) and
  faster with the short path alone (1.47, 1.48). The program calls only its own function and `print`,
  once, and uses neither `get_or` nor any of the 15 builtins. Multiplied, the two steps give 0.98 for the
  best times and 1.01 for the medians; that is a rough check, because the two factors come from separate
  runs. Three samples measured both changes in one step: the two Linux samples at the top of this section
  gave 1.00 and 1.03 (ratios of best times), and the sample of the whole step above, whose "before" build
  is 0.2.2, gave 0.93. So the verdict for this program changed with each new build, and a verdict on one
  change is not always an effect of what the change does. A guess, not tested: the place the compiler
  gives the code of the virtual machine's loop.
- The same swing, slower with the first change and faster with the second on the same system, shows for 5
  of the 6 programs judged slower with the `get_or` change alone: on Windows `bench/mandelbrot`,
  `micro/call_g_float_only` and `micro/call_h_sqrt`; on Linux `bench/loop_sum` and `micro/call_d_len`.
- With the short path alone the small programs written to time `sqrt` and `len` were judged faster
  (`micro/call_h_sqrt` on Windows, `micro/call_d_len` on Windows and Linux), although those two calls are
  instructions of their own and the two programs call none of the 15 builtins. Why was not looked into.
- This is a limit of the comparison tool. Its band for "no clear change" comes from timing one build
  twice, so it covers timing noise. It does not cover the effect that a new build can have on code the
  change did not touch. A single verdict for a program that a change does not touch should be read with
  that in mind.
- In all four reports both builds printed the same in 193 runs, gave the same 23 error messages and used
  the same fuel in 56 programs.

## Fuzz and differential tests

Added by the same separate script from the output of `python tests/fuzz_test.py` (seed 20261112, 200,000
fuel and 16 MB per run). `judge.py` does not run this test. The test and its Python reference were
written by the author of Kilat.

| Kind of case | Cases | What must hold |
|---|---|---|
| Deep or runaway programs | 46 | ends as a result or an ordinary error |
| Corpus files, damaged copies, random tokens | 6,062 | no crash, no hang, `check` agrees with `run` |
| Random Int, Float and Bool expressions | 4,000 | same answer as the Python reference |
| `mean`, `median`, `stddev` | 600 | same answer as Python's `statistics` |
| Sums of lengths, masses and times | 600 | same answer as the reference |
| Sums of, or conversions between, different dimensions | 300 | refused before running, with a unit-mismatch message |
| Total | 11,608 | 0 failures, 0 skipped |

- How the runs ended: 5,618 finished, 5,601 refused before running, 318 run-time errors, 67 fuel stops, 4
  memory stops.
- 308 of the 4,000 expressions overflow or divide by zero; each was refused at run time as expected.
- The test calls Kilat through its library, not through the program. This run used the library built on
  this PC (see "About this speed run"); GitHub's runs use the library each machine builds.
- A mistake in this test was found and corrected for this version. Before each case with units the test
  asked Kilat whether the unit exists, with a line like `print(1 mm)`. The display rule of 0.2.3 refuses
  that line, so the test took every unit for unknown and skipped all 900 cases of the last two rows
  without failing. The four GitHub runs of commit `fe73fa1` show it: 11,608 cases, 900 skipped and 0
  failures in each. The test now asks with `print((1 mm) in mm)`; an unknown unit is a failure, not a
  skip; a refusal must be a unit-mismatch refusal; and the run fails when every case of a kind was
  skipped.
- The correction was tested with planted faults: each of the 3 (a refusal that is really a syntax error,
  a unit that does not exist and a unit sum with a wrong reference) was reported as a failure, and 2
  valid cases passed. With the old question put back, a short run skipped all 12 of its unit cases, ended
  with exit code 1 and named `refusal` and `units` as skipped entirely.
- Worker restarts: 0. The run took 34 s with 4 worker processes; the longest single case took 9.2 s (the
  hang limit is 60 s). The load of the PC was sampled just before and just after the run, not during it:
  88 % and 84 % of the processors in use, 16.0 and 15.7 GB of memory free, paging at 0 and 1,507 pages
  per second. Earlier runs took 373 s for 0.2.2 (PC in use for other work), 10 s for 0.2.1 (quiet PC) and
  591 s for 0.2.0 (heavily loaded PC).
- Before the run the harness caught a planted crash and a planted hang (`--selftest`).
- One fault was found while writing the test: in 0.2.0 a fuel stop could report the wrong place (`at
  <prelude>:309 in to_celsius` for an empty `while true { }` loop). It is fixed in 0.2.1, and `python
  tests/fuel_location_test.py` checks four cases (4 of 4).
- `python tests/unit_hints_test.py` checks how values with a unit are shown and the hints Kilat gives,
  now with the display rule of 0.2.3 (75 of 75; the test had 20 cases for 0.2.2).

## Release download check (0.2.3)

Added on 2026-11-16 by a further script, which is not in the repository, from the records of two other
scripts that are not in it either: one downloaded the two files of the 0.2.3 release on 2026-11-16 and
checked them, the other holds the result of the install made the same day.

| Check | Result |
|---|---|
| `kilat.exe` from the release page | 1,362,944 bytes; SHA-256 the same as the file GitHub built, which is the program measured above |
| `kilat-windows-x64.zip` from the release page | 680,444 bytes; SHA-256 the same as the uploaded file; holds exactly the 10 expected files; the `kilat.exe` inside has the same SHA-256 as the download |
| The 9 other files in the zip | each the same as the file at tag `v0.2.3` |
| `kilat version`, with the download and with the program from the zip | `kilat 0.2.3` |
| The 5 examples, run with the program from the zip | each ends with exit code 0 and gives the same output with the downloaded `kilat.exe` and with the program that GitHub's `ci` workflow built with Microsoft's toolchain from commit `fe73fa1` |
| `print(2 m + 3 kg)` | refused before it runs: unit mismatch, cannot add m and kg |
| `let d = 2.4 mm` then `print(d)` | refused before it runs, with a hint that names `{d:mm}` |
| `let d = 2.4 mm` then `print("{d:mm}")` | prints `2.4 mm` |

- SHA-256 of `kilat.exe`: `db646696dc987dff95ebc9b64df5de275e0311cd8170479582aba07d0a0fe9cc`
- SHA-256 of `kilat-windows-x64.zip`: `6ca3a728f165acc73ef0e3b69d6650f6dd6f62a1636026ad3adbc018a8475466`
- GitHub's own digests of the two files, read from its API, are the same values.

The install, on the author's PC:

- `install.cmd /quiet` was run from the unpacked download on 2026-11-16 and ended with exit code 0.
- The installed `kilat.exe` is the file of the release (same size, same SHA-256). It replaced an
  installed 0.2.1 (1,271,808 bytes).
- The user's PATH already held the Kilat folder from the earlier install and was left unchanged: 18
  entries before and after, the same text.
- A new process, given the PATH that the registry holds after the install (machine part, then user part),
  found `kilat` in the install folder. In it `kilat version` printed `kilat 0.2.3`,
  `kilat run examples\hello.kilat` printed `stress = 20.7 MPa` and `kilat eval "print(90 km/h in mph)"`
  printed `55.9234073014`.
- Not tested: a first install of 0.2.3 on a PC without Kilat (the step that adds the folder to PATH did
  not run this time), another PC, and a download with a browser.
- The files are not signed, so Windows may show a SmartScreen prompt. The files checked here were
  downloaded by a script, not with a browser.

## Reruns on GitHub's machines (0.2.3)

Added by the same separate script from the results of `.github/workflows/ci.yml`. Each machine builds
Kilat and runs `judge.py --quick` (2 runs per program instead of 5) and the fuzz test. On Windows that
build uses Microsoft's toolchain, while the file built for the release uses the GNU toolchain. The script
and the programs are the author's; no other person repeated the measurements.

| Commit | Run | Machine | Processor | CPython | Speed, whole process | Speed, net of start-up | Slower than CPython | Bugs refused | Fuzz cases | Fuzz skipped | Fuzz failures |
|---|---|---|---|---|---|---|---|---|---|---|---|
| `fe73fa1` | 43418571144 | Windows | AMD EPYC 9V74 80-Core Processor | 3.13.17 | 3.09 | 3.02 | 0 of 10 | 20 of 20 | 11,608 | 900 | 0 |
| `fe73fa1` | 43418571144 | Linux | not reported | 3.13.18 | 2.96 | 2.91 | 0 of 10 | 20 of 20 | 11,608 | 900 | 0 |
| `fe73fa1` | 43418571020 | Windows | AMD EPYC 9V45 96-Core Processor | 3.13.17 | 2.84 | 2.76 | 0 of 10 | 20 of 20 | 11,608 | 900 | 0 |
| `fe73fa1` | 43418571020 | Linux | not reported | 3.13.18 | 3.13 | 3.06 | 0 of 10 | 20 of 20 | 11,608 | 900 | 0 |
| `858dc6b` | 43586788450 | Windows | AMD EPYC 9V74 80-Core Processor | 3.13.17 | 3.09 | 3.03 | 0 of 10 | 20 of 20 | 11,608 | 0 | 0 |
| `858dc6b` | 43586788450 | Linux | not reported | 3.13.18 | 2.98 | 2.92 | 0 of 10 | 20 of 20 | 11,608 | 0 | 0 |
| `858dc6b` | 43588103562 | Windows | AMD EPYC 9V45 96-Core Processor | 3.13.17 | 2.92 | 2.85 | 0 of 10 | 20 of 20 | 11,608 | 0 | 0 |
| `858dc6b` | 43588103562 | Linux | not reported | 3.13.18 | 3.12 | 3.06 | 0 of 10 | 20 of 20 | 11,608 | 0 | 0 |

- GitHub started the workflow twice for commit `fe73fa1` (two events for the same pull request), so each
  machine type ran it twice. Commit `858dc6b` ran once for its pull request (run 43586788450) and once
  after it reached `main` (run 43588103562). All eight runs are shown; none was picked or left out. The
  rows of commit `858dc6b` and the notes on them were added on 2026-11-16 by a further script, from
  reports of the same kind.
- Over these 8 runs Kilat was 2.84 to 3.13 times as fast as CPython for the whole process and 2.76 to
  3.06 net of start-up. 4 of the 8 runs reached 3 times for the whole process, and the same 4 reached it
  net of start-up. CPython's start-up took 10 to 24 ms of CPU time on these machines and 74 ms in the run
  on the author's PC, so the net figures of the two should not be compared directly.
- In each of the 8 runs `strings` was the program closest to CPython net of start-up, at 1.14 to 1.41;
  `sieve` gave 1.73 to 2.48.
- The 900 skipped fuzz cases in the rows of commit `fe73fa1` are the mistake described under "Fuzz and
  differential tests": those runs used the test before the correction. The four runs of commit `858dc6b`
  used the corrected test: 11,608 cases, none skipped and no failures in each, and the cases ended the
  same way as in the run on the author's PC (5,618 finished, 5,601 refused before running, 318 run-time
  errors, 67 fuel stops, 4 memory stops).
- All four Windows runs built the same `docs/kilat.wasm` as the one in the repository (786,285 bytes,
  SHA-256 `d81524ed2069618962ca0ad36c46496c0fb96d6610e84f1763aa73055b9fd905`). All four Linux runs built
  another file of the same size (SHA-256
  `adb1a6f6279c979d6cabd6e238a72207798ed77f342c0e872335a620d3f6bbdb`), also the same in all four. Why the
  two systems give different files was not looked into.
- The Cairn plugin (`main.js`) came to 392,381 bytes in the two Windows runs of commit `858dc6b` and
  392,373 in the two Linux runs. The workflow's new step, which fails when the plugin is longer than
  Cairn's limit of 400,000 characters, passed in all four.
- Language tests 7 of 7, library tests 12 of 12 and sandbox tests 6 of 6 in every run.

## Later pushes on GitHub's machines (0.2.3)

Added on 2026-11-16 by `judge_work/round8/results_apps_sections.py` (not in the repository) from the
reports of the same workflow. Four more commits reached `main` after the release tag. None of them
changed the language or the timed programs: `git diff 858dc6b..7fe08c8` lists nothing under `src/` or
`bench/`, and `judge.py` is the same. The rows have the same form as in the table above.

| Commit | Run | Machine | Processor | CPython | Speed, whole process | Speed, net of start-up | Slower than CPython | Bugs refused | Fuzz cases | Fuzz skipped | Fuzz failures |
|---|---|---|---|---|---|---|---|---|---|---|---|
| `847299e` | 43801383391 | Windows | AMD EPYC 9V45 96-Core Processor | 3.13.17 | 2.90 | 2.83 | 0 of 10 | 20 of 20 | 11,608 | 0 | 0 |
| `847299e` | 43801383391 | Linux | not reported | 3.13.18 | 3.15 | 3.09 | 0 of 10 | 20 of 20 | 11,608 | 0 | 0 |
| `9e1830b` | 43825708015 | Windows | AMD EPYC 7763 64-Core Processor | 3.13.17 | 3.11 | 3.04 | 0 of 10 | 20 of 20 | 11,608 | 0 | 0 |
| `9e1830b` | 43825708015 | Linux | not reported | 3.13.18 | 3.14 | 3.07 | 0 of 10 | 20 of 20 | 11,608 | 0 | 0 |
| `bb33f08` | 43836417469 | Windows | AMD EPYC 7763 64-Core Processor | 3.13.17 | 3.17 | 3.11 | 0 of 10 | 20 of 20 | 11,608 | 0 | 0 |
| `bb33f08` | 43836417469 | Linux | not reported | 3.13.18 | 2.94 | 2.88 | 0 of 10 | 20 of 20 | 11,608 | 0 | 0 |
| `7fe08c8` | 43837645294 | Windows | AMD EPYC 9V74 80-Core Processor | 3.13.17 | 3.19 | 3.13 | 0 of 10 | 20 of 20 | 11,608 | 0 | 0 |
| `7fe08c8` | 43837645294 | Linux | not reported | 3.13.18 | 2.78 | 2.70 | 0 of 10 | 20 of 20 | 11,608 | 0 | 0 |

- What the four commits changed: `847299e` the two documents; `9e1830b` a second workflow
  (`.github/workflows/apps.yml`), two test scripts for it and four lines of the README; `bb33f08` the
  browser test rig (`tests/web_test.py`); `7fe08c8` one step of the second workflow. Each push ran this
  workflow once. All eight runs are shown; none was picked or left out.
- Every row is Kilat 0.2.3 with 2 runs per program. In each run the outputs matched CPython's and all 10
  programs were faster than CPython.
- Over these 8 runs Kilat was 2.78 to 3.19 times as fast as CPython for the whole process and 2.70 to
  3.13 net of start-up. 5 of the 8 reached 3 times for the whole process, and the same 5 reached it net
  of start-up. Over all 16 runs of 0.2.3 on GitHub's machines the figures are 2.78 to 3.19 and 2.70 to
  3.13, and 9 of the 16 reached 3 times.
- The lowest and the highest figure of all 16 runs are both in the last run (43837645294): 2.78 for Linux
  and 3.19 for Windows. In that Linux run `sieve` gave 1.59 net of start-up, `matmul` 1.77 and `strings`
  1.14; in the seven other runs of this table `sieve` gave 2.07 to 2.62. Why that run was low was not
  looked into.
- The Linux machines built the same program file, byte for byte, in all 8 of their runs, the four of the
  table above and the four here (`target/release/kilat`, SHA-256
  `c4c7f99c5914e6aee9635b827755817471c2780c4e04446e90f58c856b07b2aa`). So the spread among the Linux
  runs, 2.78 to 3.15, does not come from the build: it is one file timed on different machines at
  different times. The Windows machines built 8 different files in their 8 runs (2.84 to 3.19); why
  Microsoft's toolchain does not give the same file twice was not looked into.
- `strings` was again the program closest to CPython net of start-up in each of the 8 runs, at 1.14 to
  1.36. `sieve` gave 1.59 to 2.62, so the range given above for the first 8 runs (1.73 to 2.48) does not
  hold for all 16. CPython's start-up took 8 to 26 ms of CPU time.
- Fuzz test in each of the 8 runs: 11,608 cases, none skipped, no failures, no worker restarted, and the
  cases ended the same way as before (5,618 finished, 5,601 refused before running, 318 run-time errors,
  67 fuel stops, 4 memory stops).
- As before, the four Windows runs built the same `docs/kilat.wasm` as the one in the repository and the
  four Linux runs built the other file of the same size. The Cairn plugin (`main.js`) came to 392,381
  bytes in the Windows runs and 392,373 in the Linux runs; the limit is 400,000.
- 20 of 20 bugs refused, language tests 7 of 7, library tests 12 of 12 and sandbox tests 6 of 6 in every
  run.

Each program in the last run (43837645294), as times CPython's speed. This is the run with the lowest and
the highest figure:

| Program | Linux, whole process | Linux, net of start-up | Windows, whole process | Windows, net of start-up |
|---|---|---|---|---|
| `collatz` | 2.93 | 2.90 | 3.35 | 3.34 |
| `fib` | 2.54 | 2.45 | 3.78 | 3.76 |
| `loop_sum` | 4.68 | 4.65 | 3.70 | 3.68 |
| `mandelbrot` | 3.46 | 3.43 | 3.12 | 3.09 |
| `map_count` | 2.29 | 2.21 | 2.45 | 2.36 |
| `matmul` | 1.87 | 1.77 | 2.30 | 2.18 |
| `nbody` | 2.87 | 2.83 | 3.72 | 3.70 |
| `sieve` | 1.67 | 1.59 | 2.66 | 2.56 |
| `sort` | 9.08 | 9.17 | 9.61 | 10.13 |
| `strings` | 1.22 | 1.14 | 1.42 | 1.27 |

## Model sample on the live GlowCode page (0.2.2)

Added by hand from `judge_work/model_sample_round7.py` (not in the repository). The script opens the
live page in Edge, types an engineering question, lets the page's free models write Kilat and counts a
run as correct only when every expected number is printed within 1.5 %. 45 s between runs.

| Run | Tasks x repeats | Correct | Correct on first try | Ran but wrong | Refused by checker | Test rig error |
|---|---|---|---|---|---|---|
| pair (the two tasks that failed in 0.2.1) | 2 x 3 | 6 of 6 | 6 | 0 | 0 | 0 |
| full (same 10 tasks as for 0.2.1) | 10 x 3 | 27 of 30 | 25 | 2 | 0 | 1 |
| held-out (4 new tasks, numbers not in the prompt) | 4 x 1 | 4 of 4 | 3 | 0 | 0 | 0 |

For 0.2.1 the same 10 tasks gave 24 of 30 correct (5 refused, 1 wrong number), with shaft torque and
spring 0 of 3 each. Both are 3 of 3 here.

Read these numbers with care:

- Replayed answers. The full run has only 15 different programs in 30 runs, and 14 runs came back in
  under 3 s, so the provider replayed stored answers. The 30 runs are not 30 independent samples.
- The prompt was taught the test. The page's model prompt now says "`rpm` is already an angular speed:
  with `let speed = 1450 rpm`, torque is `power / speed`", the same number as the shaft-torque task. The
  held-out run (960 rpm, 2900 rpm, other numbers) is the only check that it carries over, and it is 4 runs.
- Found later (2026-11-16): three of the 10 tasks (`rod stress`, `ohm`, `box volume`) are word for word
  the page's own example questions, and the prompt's line of example quantities holds `1.5 kN`, the load
  of the `rod stress` task, and `12 V` and `470 ohm`, the inputs of the `ohm` task. All of this was
  already so in the page's file for this sample (commit `efb9522` of the page's repository). See "Model
  sample on the live GlowCode page (0.2.3)" below.
- The three new checker messages of 0.2.2 were not seen to fire in these runs. The gain comes from the
  prompt lines or from model variation, not provably from the checker.
- The 2 wrong runs are one replayed program for the "readings" task. It printed
  `mean = 2.12 mm, largest = 0.0024 m`. The value is right (2.4 mm) but shown in metres, because the
  placeholder `{largest}` has no unit. The fixed rule counts it wrong and it stays wrong here. This is
  an open defect: a quantity printed with no unit shows base SI, not the unit it was entered in.
- Since then, 0.2.3 refuses a program of that kind before it runs. A program written for this check
  prints `largest = {largest}` for a value entered in mm: with 0.2.2 it printed `largest = 0.0024 m`;
  with 0.2.3 both `kilat run` and `kilat check` refuse it and suggest `{largest:mm}`. The program the
  model wrote was not run again. The page was sampled again with 0.2.3 on 2026-11-16; see "Model sample
  on the live GlowCode page (0.2.3)" below.
- The test rig error is one run where the click on the Run button timed out after 30 s. That run
  measured nothing, so 29 runs were measured in the full run.
- In the held-out run, one first try (piston force) was refused by the checker and the second try was
  accepted. The refusal message was not recorded.

## Checks of the two apps on GitHub's machines (0.2.3)

Added on 2026-11-16 by the same script, from the saved reports of a second workflow,
`.github/workflows/apps.yml`. That workflow is started by hand and never by a push. It has three jobs:
one tests the live GlowCode page, one tests the Kilat plugin inside Cairn's web app, for Windows and for
Linux, and one samples the page's free models, only when asked (next section). It was started three
times, all on 2026-11-16. All three runs are shown; none was picked or left out.

| Run | Kilat commit | Live GlowCode page | Cairn plugin, Windows | Cairn plugin, Linux | Model sample |
|---|---|---|---|---|---|
| 43825862174 | `9e1830b` | 9 of 9 | 8 of 8 (Cairn `ee87743`) | failed, no report | run |
| 43836555198 | `bb33f08` | 9 of 9 | 8 of 8 (Cairn `89567cd`) | failed, no report | not run |
| 43837700374 | `7fe08c8` | 9 of 9 | 8 of 8 (Cairn `75af214`) | 8 of 8 (Cairn `75af214`) | not run |

### The live GlowCode page

`tests/glowcode_live_test.py` opens https://juw12.github.io/projects/glow/code/ in Playwright's own
Chromium (159.0.8154.6) on one of GitHub's Linux machines. It taps each of the page's five example
buttons, types four more programs, presses Run each time and reads what the page shows. No model is used.
In each of the three runs the page said "Kilat 0.2.3 is ready. Each run may use 20,000,000 steps and 32
MB.", and the `kilat.wasm` it served was the repository's `docs/kilat.wasm`, byte for byte (786,285
bytes, SHA-256 `d81524ed2069618962ca0ad36c46496c0fb96d6610e84f1763aa73055b9fd905`). A case passes only
when the program ends in the stated way and the page shows each stated text. All nine passed in each run:

| Case | Entered by | Must end as | Text that must show |
|---|---|---|---|
| example 1: Stress in a bar | example button | finished | `stress = 20.8 MPa` |
| example 2: Gap statistics | example button | finished | `mean   2.10 mm`, `median 2.05 mm`, `spread 0.216 mm` |
| example 3: A unit mistake | example button | refused before running | `cannot add m and kg` |
| example 4: A function and a loop | example button | finished | the lines `0`, `1`, `1`, `2`, `3`, `5`, `8`, `13`, `21`, `34` |
| example 5: A loop that never ends | example button | stopped at the work limit | `work limit` |
| spring, units inside the braces | typed | finished | `96.0 N`, `0.384 J` |
| shaft torque from power / speed with rpm | typed | finished | `49.4 N*m` |
| rpm times 2 * pi / 60 is refused | typed | refused before running | `rpm` |
| unit word after a placeholder is refused | typed | refused before running | `{force:N}` |

### The Kilat plugin in Cairn's web app

`tests/cairn_app_test.py` takes a fresh checkout of Cairn's `main` (github.com/juw12/cairn) and copies
its `docs` folder, which is Cairn's web app, to a work folder. It puts the plugin's `main.js` beside
Cairn's own plugins and opens the app with its demo brain in headless Edge. The page
`tests/cairn_app_runner.html` then installs the plugin through Cairn's own plugin loader. Nothing in the
checkout is changed.

- Windows, with Edge 156: all eight checks passed in each of the three runs. Cairn's `main` moved between
  the runs, so they tested three different Cairn commits (`ee87743`, `89567cd` and `75af214`). The last
  check was done 1.3 s, 1.6 s and 1.1 s after the page opened. The eight checks:
  - Cairn's web app starts with its demo brain
  - Cairn's own loader installs main.js
  - Kilat's commands are in Cairn's command list
  - Cairn's Markdown renderer hands a kilat block to the plugin
  - a note with kilat blocks, opened in Cairn, shows the results
  - Cairn's Plugins page lists Kilat
  - after a reload Cairn starts the plugin again from its own storage
  - turning the plugin off and removing it leaves Cairn clean
- Linux, run 43825862174: failed. The test page did not report within 150 s, and the test rig did not yet
  record what the browser had done.
- Linux, run 43836555198: failed again, this time with a reason. Commit `bb33f08` had made the rig
  (`tests/web_test.py`) record what the browser does. Edge ended with exit code -6 before it asked for
  any page ("requests answered: 0"), with the message "The SUID sandbox helper binary was found, but is
  not configured correctly".
- Linux, run 43837700374: 8 of 8 with Edge 157, against Cairn `75af214`. Commit `7fe08c8` had added one
  step that runs for the Linux machine only,
  `sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0`. The job's log shows that setting go
  from 1 to 0 before the test. The rig does not pass `--no-sandbox`, so Edge's own sandbox stayed on. The
  last check was done 6.0 s after the page opened.
- So for Linux the plugin passed once, after two failures. That is not yet a steady result. The reading
  that Ubuntu's setting kept Edge from building its sandbox fits the message of the second run and the
  pass of the third, but it rests on that one run, and the step needs `sudo` on GitHub's machine.

### What these checks do not cover

- Cairn's desktop build, and installing the plugin from GitHub inside Cairn: the test puts `main.js` in
  place itself.
- The GlowCode page at phone width or offline.
- Kilat's own browser checks. `tests/web_test.py` and `tests/page_test.py` are run by no workflow, and
  `judge.py` skips its browser part on GitHub: none of the 16 reports of the two tables of GitHub's runs
  above holds a browser result. Those checks have run only on the author's PC.

## Model sample on the live GlowCode page (0.2.3)

Added on 2026-11-16 by the same script, from the reports of the two `model-sample` jobs of run
43825862174 of `.github/workflows/apps.yml` (the first run in the section above).
`tests/glowcode_model_sample.py`, in the repository since `9e1830b`, opens the live page in Playwright's
own Chromium (159.0.8154.6) on one of GitHub's Linux machines. For each run it types an engineering
question in plain words and lets the page's free models write the Kilat program. The page gives the
models up to three tries and hands the checker's message back after each refusal. A run counts as correct
only when every expected number is among the numbers the program printed, within 1.5 %. The script waits
45 s between runs. The main set ran from 07:44 to 08:15 UTC and the held-out set from 08:16 to 08:59 UTC.
In every run the page said "Kilat 0.2.3 is ready. Each run may use 20,000,000 steps and 32 MB." Its
`code.js` was 17,550 bytes (SHA-256 `776382afcd1abae1f9589c6c6f534e1a40abecbfbfdb7186e3769bd62dbd63ee`)
and its prompt for the models was 35 lines (SHA-256
`d272d4630666f858a14d6abfa417f6ecd1facbe1147c75ffa596292b502c86ec`), the same in both sets.

| Run | Tasks x repeats | Correct | Correct on first try | Ran but wrong | Refused by checker | Test rig error |
|---|---|---|---|---|---|---|
| main (same 10 tasks as for 0.2.1 and 0.2.2) | 10 x 3 | 30 of 30 | 26 | 0 | 0 | 0 |
| held-out (14 tasks: the 4 held-out tasks of the 0.2.2 sample and 10 new ones) | 14 x 3 | 39 of 42 | 36 | 1 | 2 | 0 |

Of the 72 runs, 69 printed every expected number. One printed the right amount in millilitres where
litres were asked. Two ended with no program accepted in three tries; the page then says that nothing was
run, and it shows no number. No other language was given these questions, so the sample says nothing
about how Kilat compares with Python or anything else.

On the same ten questions the 0.2.2 sample gave 27 of 30 and the 0.2.1 sample 24 of 30 (section above).
The samples differ in more than the checker: the page's prompt for the models was changed along with the
engine, the free models behind the page change over time, and the 0.2.2 sample was taken in Edge on one
PC. So the first row says where the page stood on 2026-11-16. It does not measure what the 0.2.3 rule
did.

Before a program was accepted, or the three tries ran out, the checker refused 17 programs: 5 in 4 runs
of the main set and 12 in 5 runs of the held-out set. Its messages, by their first line:

| Set | Checker's message (first line) | Programs refused | Where |
|---|---|---|---|
| main | `` cannot read the format `m/s:.2` `` | 2 | average speed: repeat 1 tries 1 and 2 |
| main | `` cannot assign to `sum`: it is not a `var` `` | 1 | sum to 100: repeat 1 try 1 |
| main | `` cannot read the format `N.m.1` `` | 2 | shaft torque: repeat 2 try 1, repeat 3 try 1 |
| held-out | `unit mismatch: cannot show Pa as kN` | 4 | piston force: repeat 1 try 1, repeat 2 tries 1 and 3, repeat 3 try 1 |
| held-out | `unit mismatch: cannot show Float as kN` | 1 | piston force: repeat 1 try 2 |
| held-out | `this value has a unit, so say which unit to show it in` | 2 | piston force: repeat 2 try 2, repeat 3 try 2 |
| held-out | `unit mismatch: cannot show m^6 as L` | 3 | box volume 2: repeat 2 tries 1 and 2, repeat 3 try 1 |
| held-out | `` unexpected `/` after the end of the statement `` | 1 | box volume 2: repeat 3 try 2 |
| held-out | `unit mismatch: cannot show Float as L` | 1 | box volume 2: repeat 3 try 3 |

### The refusals in the main set

All four runs with a refusal ended correct, on the second or third try. Three of the five refused
programs spelled a print format in a way the checker cannot read, and one assigned to a name that was not
declared as a variable.

- The question "average speed", repeat 1: Meta-Llama-3_3-70B-Instruct sent the same program twice, with
  `{speed:m/s:.2}`. The checker's hint gives three example formats (`{x:.2}`, `{x:8}` and `{p:MPa.1}`);
  none of them has a unit with a slash in it. The third program, from gpt-oss-20b, has `{speed:m/s.2}`
  and was accepted.
- The question "shaft torque", repeats 2 and 3: gpt-oss-20b wrote `{torque:N.m.1}` and got the same hint.
  The next program had `{torque:N*m.1}` and was accepted; it came from Meta-Llama-3_3-70B-Instruct in
  repeat 2 and from gpt-oss-20b in repeat 3. In repeat 1 the first program, from
  Meta-Llama-3_3-70B-Instruct, was accepted.
- The question "sum to 100", repeat 1: the program declared `let sum = 0` and then assigned to it. The
  checker said that `sum` is not a `var` and how to declare one. The second program, from minimax-m2.7,
  has `var sum = 0`.

### The two held-out questions that went wrong

The other 12 held-out questions were correct on the first try in all three repeats (36 runs). The three
misses, and the three correct runs that needed three tries, belong to two questions.

**Piston force.** The question is "A pressure of 2.5 MPa acts on a round piston 80 mm in diameter. Print
the force in kN with two decimals." The expected number is 12.57.

- In all three repeats the first program, from gpt-oss-20b, was the same text. It has the line
  `let radius = diameter / 2.0 mm`. A length divided by a length is a plain number, so the area has no
  unit and the value named `force` is still a pressure. The checker refused `{force:kN.2}` with "unit
  mismatch: cannot show Pa as kN". The message is true, but it points at the print and not at the line
  where the unit was lost.
- Repeat 1: the second program, from Meta-Llama-3_3-70B-Instruct, turned the force into a plain number
  (`pressure * area in kN`) and still asked for `{force:kN.2}`. It was refused with the hint to put the
  unit after the braces. The third program has `{force in kN:.2} kN` and printed `Force = 12.57 kN`.
  Correct.
- Repeat 2: the second program has `diameter / 2.0 in mm`, a plain number again, and
  `{force / 1 kN:.2} kN`. The rule that is new in 0.2.3 refused it ("this value has a unit, so say which
  unit to show it in") with the hint `` write `{force / 1 kN:MPa/kN.2}` ``. The hint is true of the
  value, which is a pressure divided by a force, but it does not lead to the mistake. The third program
  was refused like the first one. The run ended refused, and nothing was run.
- Repeat 3: the second program got the same message and the same hint. The third one kept the unit of the
  radius (`let radius = diameter / 2`) and copied the shape of the hint: `{force / 1 kN:kN/kN.2} kN`. It
  printed `Force = 12.57 kN/kN kN`. The number is right and the label is not. The test reads numbers
  only, so this run is one of the 39 counted as correct.

**Box volume 2.** The question is "A box is 450 mm by 280 mm by 120 mm. Print its volume in litres." The
expected number is 15.12.

- Repeat 1: one program, from Meta-Llama-3_3-70B-Instruct, accepted at once. It has `{volume:mL.2}` and
  printed `volume = 15120.00 mL`. That is the right volume, in millilitres where litres were asked. The
  test counts it as wrong, because 15.12 is not among the printed numbers.
- Repeat 2: the first two programs multiplied a volume by `1 L` (`volume_mm3 / 1e6 * 1 L` and
  `volume / 1e6 * 1 L`), which is a volume times a volume. Both were refused with "unit mismatch: cannot
  show m^6 as L". The third program, from codestral-latest, has `let volume_l = volume in L` and
  `{volume_l:.2} L`. It printed `volume = 15.12 L`. Correct.
- Repeat 3: the first program was the same as in repeat 2. The second has the line
  `let volume_L = volume_mm3 in mL / 1000`. The checker took the statement to end at `mL` and stopped at
  the `/` (`` unexpected `/` after the end of the statement ``, with the hint "put each statement on its
  own line"). The third program did the division on a line of its own and then asked for `{volume_L:L.2}`
  of what was by then a plain number. It was refused, and the run ended refused.

### How to read these numbers

- It is one sample of each set, taken on one day from one machine, of free models that change. It was not
  repeated, and it is not a rate to rely on.
- The runs of one question are not independent. In the main set 15 of the 30 runs came back in under 3 s
  (2 of them a first repeat), the 30 runs ended with 14 different programs, and in 16 of the 20 second
  and third repeats the last program was, character for character, the last program of an earlier repeat
  of the same question. In the held-out set: 24 of 42 under 3 s (none of them a first repeat), 21
  different programs, and 21 of 28. As in the 0.2.2 sample, stored answers seem to be played back; where
  they are stored was not looked into. A run took between 0.5 and 62.0 s.
- Three of the ten main questions (rod stress, ohm and box volume) are, word for word, the three example
  questions that the page itself offers as buttons. The page's prompt for the models names `1.5 kN`,
  `12 V` and `470 ohm` among its examples of quantities: the load of "rod stress" and the two inputs of
  "ohm". It uses `1450 rpm` in its line on torque, the speed of "shaft torque". The main set is therefore
  the easier of the two.
- No expected answer of either set is among the numbers in that prompt. Numbers typed in the questions
  are: in 8 of the 10 main questions and 5 of the 14 held-out ones, at least one number of the question
  is also somewhere in the prompt. For the main set these are the four numbers named above (1.5, 12, 470
  and 1450) and the small numbers 1, 2, 4 and 60; "spring" has its 12 as a rate in N/mm, so it shares
  that number with the prompt's `12 V` by chance. For the held-out set these are the small numbers 1, 3,
  4 and 60 only.
- Four of the 14 held-out questions are not new: motor torque, spring 2, shaft power and piston force
  were the held-out questions of the 0.2.2 sample. All 24 questions were fixed in `9e1830b`, before the
  sample was started, and that file was not changed afterwards.
- The test reads numbers, not labels. `Force = 12.57 kN/kN kN` counted as correct. So did two main runs
  of "readings" that printed `mean = 2.12` and `max = 2.4` with no unit at all. It reads every number in
  the output: in `second moment of area = 720000.0 mm^4` it reads 720000 and 4.
- In the two "readings" questions the largest reading is one of the numbers typed in the question, so a
  program that only printed its inputs back would show that expected number.
- The other way round, a right amount in another unit counts as wrong (`volume = 15120.00 mL`).
- The 0.2.3 rule (a value with a unit is not printed without a named unit) refused 2 of the 17 refused
  programs, both in "piston force", and none in the main set. In the two "readings" questions, where the
  0.2.2 sample had its wrong answer (`largest = 0.0024 m`), all six runs were correct. That is what was
  seen; with a new prompt and replayed answers it does not show that the rule is the cause.
- Programs written, by model as the page names them: main set gpt-oss-20b 21, Meta-Llama-3_3-70B-Instruct
  13 and minimax-m2.7 1; held-out set gpt-oss-20b 39, Meta-Llama-3_3-70B-Instruct 11 and codestral-latest
  2.
- It is a measurement, not a gate. The job is started by hand and never by a push, and it ends green
  whatever the counts are: the script fails only when no run reaches the page.

### Follow-ups this sample suggests (not done)

- A clearer message for a print format that cannot be read (`N.m.1`, `m/s:.2`). It could say how the unit
  is written (`N*m`, `m/s`) and that the decimals follow one dot.
- For a value of another kind than the unit asked ("unit mismatch: cannot show Pa as kN", "unit mismatch:
  cannot show m^6 as L"), name the line where the value got its unit. In 6 of these 7 programs the unit
  was lost or doubled on an earlier line than the print the message points at.
- The hint `` write `{force / 1 kN:MPa/kN.2}` `` is true but did not help: a model followed it to
  `kN/kN`. For a value divided by one unit, the hint could be the value itself in that unit.
  `{force:kN.2}` prints `Force = 12.57 kN` when the force is a force (checked by hand with the installed
  0.2.3), and it gets the "unit mismatch: cannot show Pa as kN" message when it is not.
- For `x in mL / 1000` the hint speaks of statements on one line. With brackets,
  `(volume_mm3 in mL) / 1000`, the same program is accepted and prints `volume = 15.12 L` (checked by
  hand with the installed 0.2.3). The hint could say so.
- The first mistake in all three repeats of "piston force" was `diameter / 2.0 mm`. The page's prompt
  says that `x in J` gives a plain number; neither it nor a checker message says the same of a division
  by a quantity.
- The test could accept a right amount in another unit and could check the unit label, so that
  `15120.00 mL` and `12.57 kN/kN kN` are each judged for what they are.
