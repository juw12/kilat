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

- The program measured is the file built for the 0.2.3 release: `kilat.exe`, 1,362,944 bytes, SHA-256
  `db646696dc987dff95ebc9b64df5de275e0311cd8170479582aba07d0a0fe9cc`. GitHub's Windows machine built it
  with the GNU toolchain from commit `fe73fa1` (run 43418571070, the comparison of the next section).
- GitHub built that commit twice in that run (attempts 1 and 2; the second is a rerun asked for by the
  author to get a second sample). The two programs have the same size and differ in 4 bytes, all inside
  the time-stamp and checksum fields of the file header. The measured file is the first build. GitHub's
  run page now lists only the files of the second build; the first build is kept as the author's copy and
  is the file meant for the release.
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
  fell from 0.313 s to 0.221 s while Kilat's went from 0.123 s to 0.132 s. GitHub's four runs gave 2.06
  to 2.43 net. Why Kilat's time did not fall with CPython's was not found. One guess, not tested: the
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

Both changes were kept. Not measured: each of the two changes on its own, and the speed of the web engine
(`docs/kilat.wasm`).

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

- GitHub started the workflow twice for commit `fe73fa1` (two events for the same pull request), so each
  machine type ran it twice. Both runs are shown; neither was picked.
- Over these 4 runs Kilat was 2.84 to 3.13 times as fast as CPython for the whole process and 2.76 to
  3.06 net of start-up. CPython's start-up took 11 to 24 ms of CPU time on these machines and 74 ms in
  the run on the author's PC, so the net figures of the two should not be compared directly.
- The 900 skipped fuzz cases in the rows of commit `fe73fa1` are the mistake described under "Fuzz and
  differential tests": those runs used the test before the correction.
- Language tests 7 of 7, library tests 12 of 12 and sandbox tests 6 of 6 in every run.

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
- The three new checker messages of 0.2.2 were not seen to fire in these runs. The gain comes from the
  prompt lines or from model variation, not provably from the checker.
- The 2 wrong runs are one replayed program for the "readings" task. It printed
  `mean = 2.12 mm, largest = 0.0024 m`. The value is right (2.4 mm) but shown in metres, because the
  placeholder `{largest}` has no unit. The fixed rule counts it wrong and it stays wrong here. This is
  an open defect: a quantity printed with no unit shows base SI, not the unit it was entered in.
- Since then, 0.2.3 refuses a program of that kind before it runs. A program written for this check
  prints `largest = {largest}` for a value entered in mm: with 0.2.2 it printed `largest = 0.0024 m`;
  with 0.2.3 both `kilat run` and `kilat check` refuse it and suggest `{largest:mm}`. The program the
  model wrote was not run again, and the page has not been sampled again with 0.2.3.
- The test rig error is one run where the click on the Run button timed out after 30 s. That run
  measured nothing, so 29 runs were measured in the full run.
- In the held-out run, one first try (piston force) was refused by the checker and the second try was
  accepted. The refusal message was not recorded.
