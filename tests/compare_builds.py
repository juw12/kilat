"""Compares two builds of the kilat program on one machine, in one run.

    python tests/compare_builds.py --before old/kilat.exe --after new/kilat.exe

Sections (choose with --sections, all by default):

    outputs  every program in tests/, bench/, examples/, bugs/ and compare/ goes
             through `run` and `check` (and `test` when it has test blocks) with
             both builds; exit code, printed text and error text must be equal.
    errors   run-time errors of abs and the rounding builtins: both builds must
             stop with the expected message on the expected line.
    fuel     the smallest --fuel that lets a program finish is found with the
             "before" build; at that value, one below it and half of it the two
             builds must behave the same, so the fuel accounting did not move.
    timing   bench/ and compare/micro/ are timed in three arms per round:
             before, after, and the before build once more. The third arm is the
             control: it shows how far two runs of the very same build differ on
             this machine. A change only counts when it is clearly larger.

The timing verdict follows a rule that was fixed before anything was measured:

    band    the larger of 3 % and twice the control difference
    faster  the best-time ratio and the median ratio are both above 1 + band
    slower  both are below 1 / (1 + band)
    else    "no clear change"

It writes compare.json and prints a summary. This is a report: the exit code
is 0 unless --strict is given and outputs, errors or fuel found a difference.
Standard library only.
"""

import argparse
import collections
import hashlib
import itertools
import json
import os
import platform
import re
import statistics
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT))
import judge  # noqa: E402  the same pinning and CPU-time measurement as the judge

RUN_TIMEOUT = 300  # seconds for one run of one program
SMALLEST_CHANGE = 0.03  # a speed change below 3 % is never called a change
FUEL_START = 1024
FUEL_CEILING = 2**40
SECTIONS = ("outputs", "errors", "fuel", "timing")
ARMS = ("before", "after", "again")
REPO_GROUPS = ("tests", "bench", "examples", "bugs")
COMPARE_GROUPS = ("micro", "edge")
START_UP = "micro/empty"
HAS_TEST_BLOCK = re.compile(r'^\s*test\s+"', re.MULTILINE)
HIDDEN = 0x08000000 if sys.platform == "win32" else 0  # no console window on Windows

Build = collections.namedtuple("Build", "label path version sha256 size")
Program = collections.namedtuple("Program", "name group folder argument source")

unlimited_runs = {}  # (build label, program name) -> result of `run` without a fuel limit


def run_plain(program_path, arguments, folder):
    """(exit code, printed bytes, error bytes); the exit code is None after a timeout."""
    try:
        finished = subprocess.run(
            [str(program_path), *arguments],
            cwd=folder,
            capture_output=True,
            timeout=RUN_TIMEOUT,
            creationflags=HIDDEN,
        )
    except subprocess.TimeoutExpired:
        return None, b"", b"timeout"
    return finished.returncode, finished.stdout, finished.stderr


def text_of(raw):
    return raw.decode("utf-8", "replace").replace("\r\n", "\n")


def describe_build(label, path):
    path = Path(path).resolve()
    _code, printed, _errors = run_plain(path, ["--version"], ROOT)
    return Build(
        label=label,
        path=path,
        version=text_of(printed).strip(),
        sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
        size=path.stat().st_size,
    )


def wanted(name, only):
    return not only or any(piece in name for piece in only)


def find_programs(only):
    programs = []
    for group in REPO_GROUPS:
        for path in sorted((ROOT / group).glob("*.kilat")):
            source = path.read_text(encoding="utf-8")
            programs.append(Program(f"{group}/{path.stem}", group, ROOT, f"{group}/{path.name}", source))
    for group in COMPARE_GROUPS:
        for path in sorted((ROOT / "compare" / group).glob("*.kilat")):
            source = path.read_text(encoding="utf-8")
            programs.append(Program(f"{group}/{path.stem}", group, ROOT / "compare", f"{group}/{path.name}", source))
    return [program for program in programs if wanted(program.name, only)]


def unlimited_run(build, program):
    key = (build.label, program.name)
    if key not in unlimited_runs:
        unlimited_runs[key] = run_plain(build.path, ["run", program.argument], program.folder)
    return unlimited_runs[key]


def runs_the_same(before, after, program):
    """True when the program ends with exit code 0 and both builds agree on everything."""
    result = unlimited_run(before, program)
    return result[0] == 0 and unlimited_run(after, program) == result


def first_difference(before_text, after_text):
    before_lines = text_of(before_text).split("\n")
    after_lines = text_of(after_text).split("\n")
    for index in range(max(len(before_lines), len(after_lines))):
        left = before_lines[index] if index < len(before_lines) else "<nothing>"
        right = after_lines[index] if index < len(after_lines) else "<nothing>"
        if left != right:
            return {"line": index + 1, "before": left[:200], "after": right[:200]}
    return None


def describe_difference(old, new):
    difference = {"exit": [old[0], new[0]]}
    if old[1] != new[1]:
        difference["printed"] = first_difference(old[1], new[1])
    if old[2] != new[2]:
        difference["errors"] = first_difference(old[2], new[2])
    return difference


# ------------------------------------------------------------------ outputs

def compare_outputs(before, after, programs):
    compared = 0
    different = []
    broken = []
    for program in programs:
        commands = ["run", "check"]
        if HAS_TEST_BLOCK.search(program.source):
            commands.append("test")
        for command in commands:
            if command == "run":
                old = unlimited_run(before, program)
                new = unlimited_run(after, program)
                if program.group in COMPARE_GROUPS and old[0] != 0:
                    # A program written for this comparison has to run; if it does
                    # not, the program is wrong, and timing it would mean nothing.
                    broken.append({"program": program.name, "exit": old[0], "errors": text_of(old[2])[:400]})
            else:
                old = run_plain(before.path, [command, program.argument], program.folder)
                new = run_plain(after.path, [command, program.argument], program.folder)
            compared += 1
            if old != new:
                different.append({"program": program.name, "command": command, **describe_difference(old, new)})
    return {"compared": compared, "different": different, "broken": broken}


# ------------------------------------------------------------------- errors

def error_cases():
    """(name, source, message, line) of run-time errors that must stay as they are."""
    cases = [("abs_of_smallest_int", "print(abs(-9223372036854775807 - 1))\n", "integer overflow", 1)]
    not_a_number = (("inf", "huge", "inf"), ("minus_inf", "-huge", "-inf"), ("nan", "huge - huge", "nan"))
    for function in ("floor", "ceil", "round", "trunc", "int"):
        cases.append((f"{function}_too_large", f"print({function}(1e19))\n", "1e19 is too large for an Int", 1))
        for label, argument, shown in not_a_number:
            source = f"let huge = 1e308 * 10.0\nprint({function}({argument}))\n"
            cases.append((f"{function}_{label}", source, f"{shown} cannot be turned into an Int", 2))
    cases.append(
        ("int_just_too_large", "print(int(9223372036854775808.0))\n", "9.22337203685e18 is too large for an Int", 1)
    )
    cases.append(
        ("ceil_just_too_small", "print(ceil(-9223372036854777856.0))\n", "-9.22337203685e18 is too large for an Int", 1)
    )
    return cases


def compare_errors(before, after, only):
    cases = [case for case in error_cases() if wanted("error/" + case[0], only)]
    wrong = []
    with tempfile.TemporaryDirectory() as folder:
        for name, source, message, line in cases:
            file_name = f"{name}.kilat"
            (Path(folder) / file_name).write_text(source, encoding="utf-8", newline="\n")
            expected = f"runtime error: {message}\n  at {file_name}:{line}\n"
            for build in (before, after):
                code, printed, errors = run_plain(build.path, ["run", file_name], folder)
                if (code, printed, text_of(errors)) != (1, b"", expected):
                    wrong.append(
                        {
                            "case": name,
                            "build": build.label,
                            "expected": expected,
                            "exit": code,
                            "printed": text_of(printed)[:200],
                            "errors": text_of(errors)[:400],
                        }
                    )
    return {"cases": len(cases), "wrong": wrong}


# --------------------------------------------------------------------- fuel

def run_with_fuel(build, program, fuel):
    return run_plain(build.path, ["run", program.argument, "--fuel", str(fuel)], program.folder)


def smallest_fuel(build, program, finished):
    """The smallest --fuel with which the program ends like the unlimited run, or None."""
    enough = FUEL_START
    while run_with_fuel(build, program, enough) != finished:
        enough *= 2
        if enough > FUEL_CEILING:
            return None
    too_little = enough // 2 if enough > FUEL_START else 0  # 0 is never tried: it may mean "no limit"
    while enough - too_little > 1:
        middle = (enough + too_little) // 2
        if run_with_fuel(build, program, middle) == finished:
            enough = middle
        else:
            too_little = middle
    return enough


def compare_fuel(before, after, programs):
    values = {}
    different = []
    skipped = []
    for program in programs:
        if not runs_the_same(before, after, program):
            skipped.append({"program": program.name, "why": "does not end with exit code 0 the same way in both builds"})
            continue
        least = smallest_fuel(before, program, unlimited_run(before, program))
        if least is None:
            skipped.append({"program": program.name, "why": f"still stopped with --fuel {FUEL_CEILING}"})
            continue
        values[program.name] = least
        # Finishing at `least` and stopping at `least - 1` in both builds shows that
        # the "after" build needs exactly the same amount of fuel.
        for fuel in sorted({least, least - 1, least // 2}):
            if fuel < 1:
                continue
            old = run_with_fuel(before, program, fuel)
            new = run_with_fuel(after, program, fuel)
            if old != new:
                different.append({"program": program.name, "fuel": fuel, **describe_difference(old, new)})
    return {"programs": len(values), "values": values, "different": different, "skipped": skipped}


# ------------------------------------------------------------------- timing

def judge_times(times):
    """Best, median, ratios and the verdict for one program; times are CPU seconds per arm."""
    best = {arm: min(values) for arm, values in times.items()}
    middle = {arm: statistics.median(values) for arm, values in times.items()}
    row = {
        "best_ms": {arm: round(best[arm] * 1000, 3) for arm in ARMS},
        "median_ms": {arm: round(middle[arm] * 1000, 3) for arm in ARMS},
        "runs": len(times["before"]),
    }
    if min(best.values()) <= 0:
        row["verdict"] = "too short to time"
        return row
    ratio_best = best["before"] / best["after"]
    ratio_median = middle["before"] / middle["after"]
    control_best = best["before"] / best["again"]
    control_median = middle["before"] / middle["again"]
    band = max(SMALLEST_CHANGE, 2 * max(abs(control_best - 1), abs(control_median - 1)))
    if min(ratio_best, ratio_median) > 1 + band:
        verdict = "faster"
    elif max(ratio_best, ratio_median) < 1 / (1 + band):
        verdict = "slower"
    else:
        verdict = "no clear change"
    row.update(
        {
            "ratio_best": round(ratio_best, 4),
            "ratio_median": round(ratio_median, 4),
            "control_best": round(control_best, 4),
            "control_median": round(control_median, 4),
            "band": round(band, 4),
            "verdict": verdict,
        }
    )
    return row


def time_programs(before, after, programs, rounds, cycles_per_second):
    paths = {"before": before.path, "after": after.path, "again": before.path}
    orders = list(itertools.permutations(ARMS))
    rows = {}
    skipped = []
    with judge.Pinned():
        for program in programs:
            if not runs_the_same(before, after, program):
                skipped.append({"program": program.name, "why": "does not end with exit code 0 the same way in both builds"})
                continue
            expected = text_of(unlimited_run(before, program)[1])
            times = {arm: [] for arm in ARMS}
            problem = None
            for round_number in range(rounds):
                # The order of the arms changes every round, so no arm is always
                # first (cold) or always last (warm).
                for arm in orders[round_number % len(orders)]:
                    command = [str(paths[arm]), "run", program.argument]
                    cpu, _wall, printed, code = judge.run_measured(command, program.folder, cycles_per_second)
                    if code != 0 or printed != expected:
                        problem = f"the {arm} arm ended with exit code {code} or printed something else"
                        break
                    times[arm].append(cpu)
                if problem:
                    break
            if problem:
                skipped.append({"program": program.name, "why": problem})
            else:
                rows[program.name] = judge_times(times)
    add_net_ratios(rows)
    return {
        "rounds": rounds,
        "smallest_change": SMALLEST_CHANGE,
        "programs": rows,
        "skipped": skipped,
        "geometric_means": geometric_means(rows),
    }


def add_net_ratios(rows):
    """Ratio with the start-up time (the empty program) taken off both sides."""
    start = rows.get(START_UP)
    if not start:
        return
    for name, row in rows.items():
        if name == START_UP or "ratio_best" not in row:
            continue
        net_before = row["best_ms"]["before"] - start["best_ms"]["before"]
        net_after = row["best_ms"]["after"] - start["best_ms"]["after"]
        if net_before > 0.5 and net_after > 0.5:
            row["ratio_net"] = round(net_before / net_after, 4)


def geometric_means(rows):
    means = {}
    for group in ("bench", "micro"):
        chosen = [
            row
            for name, row in rows.items()
            if name.startswith(group + "/") and name != START_UP and "ratio_best" in row
        ]
        if chosen:
            means[group] = {
                "programs": len(chosen),
                "ratio_best": round(judge.geometric_mean([row["ratio_best"] for row in chosen]), 4),
                "ratio_median": round(judge.geometric_mean([row["ratio_median"] for row in chosen]), 4),
                "control_best": round(judge.geometric_mean([row["control_best"] for row in chosen]), 4),
                "control_median": round(judge.geometric_mean([row["control_median"] for row in chosen]), 4),
            }
    return means


# ------------------------------------------------------------------- report

def notice(title, lines):
    """A summary block; on GitHub it also becomes a notice that the API can read."""
    print(f"\n{title}")
    for line in lines:
        print(f"  {line}")
    if os.environ.get("GITHUB_ACTIONS"):
        message = "\n".join(lines).replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")
        print(f"::notice title={title}::{message}")
    sys.stdout.flush()


def timing_lines(timing):
    lines = []
    for group, means in timing["geometric_means"].items():
        lines.append(
            f"{group}: geometric mean before/after {means['ratio_best']:.3f} (best), "
            f"{means['ratio_median']:.3f} (median); control {means['control_best']:.3f}, "
            f"{means['control_median']:.3f}; {means['programs']} programs"
        )
    for name, row in timing["programs"].items():
        if "ratio_best" not in row:
            lines.append(f"{name}: {row['verdict']}")
            continue
        net = f", net {row['ratio_net']:.3f}" if "ratio_net" in row else ""
        lines.append(
            f"{name}: {row['best_ms']['before']:.1f} -> {row['best_ms']['after']:.1f} ms, "
            f"ratio {row['ratio_best']:.3f} best / {row['ratio_median']:.3f} median{net}, "
            f"control {row['control_best']:.3f} / {row['control_median']:.3f}, "
            f"band {row['band'] * 100:.1f} %: {row['verdict']}"
        )
    for entry in timing["skipped"]:
        lines.append(f"{entry['program']}: not timed, {entry['why']}")
    return lines


def main():
    parser = argparse.ArgumentParser(description="Compare two builds of kilat: outputs, errors, fuel and speed.")
    parser.add_argument("--before", required=True, help="the kilat program to compare against")
    parser.add_argument("--after", required=True, help="the kilat program with the change")
    parser.add_argument("--out", default="compare.json", help="where to write the full report")
    parser.add_argument("--only", default="", help="pieces of program names, comma-separated, e.g. bench/,micro/map")
    parser.add_argument("--sections", default=",".join(SECTIONS), help="comma-separated: " + ", ".join(SECTIONS))
    parser.add_argument("--rounds", type=int, default=9, help="timing rounds; each round runs all three arms")
    parser.add_argument("--strict", action="store_true", help="exit code 1 when outputs, errors or fuel differ")
    arguments = parser.parse_args()

    for path in (arguments.before, arguments.after):
        if not Path(path).is_file():
            parser.error(f"{path} is not a file")
    sections = [name.strip() for name in arguments.sections.split(",") if name.strip()]
    unknown = [name for name in sections if name not in SECTIONS]
    if unknown or arguments.rounds < 1:
        parser.error(f"unknown section {unknown}" if unknown else "--rounds must be at least 1")
    only = [piece.strip() for piece in arguments.only.split(",") if piece.strip()]

    before = describe_build("before", arguments.before)
    after = describe_build("after", arguments.after)
    programs = find_programs(only)
    processor_name, cycles_per_second = judge.processor()
    report = {
        "before": {**before._asdict(), "path": str(before.path)},
        "after": {**after._asdict(), "path": str(after.path)},
        "machine": {
            "processor": processor_name,
            "platform": platform.platform(),
            "python": platform.python_version(),
        },
        "only": only,
    }
    notice(
        "compare builds",
        [
            f"before: {before.version}, {before.size} bytes, sha256 {before.sha256[:16]}",
            f"after: {after.version}, {after.size} bytes, sha256 {after.sha256[:16]}",
            f"machine: {processor_name}, {platform.platform()}, Python {platform.python_version()}",
        ],
    )

    failed = False
    if "outputs" in sections:
        outputs = compare_outputs(before, after, programs)
        report["outputs"] = outputs
        failed = failed or bool(outputs["different"] or outputs["broken"])
        lines = [f"{outputs['compared']} runs compared, {len(outputs['different'])} different, {len(outputs['broken'])} broken"]
        lines += [f"different: {entry['program']} ({entry['command']}) exit {entry['exit']}" for entry in outputs["different"]]
        lines += [f"broken: {entry['program']} exit {entry['exit']}: {entry['errors'][:160]}" for entry in outputs["broken"]]
        notice("compare outputs", lines)

    if "errors" in sections:
        errors = compare_errors(before, after, only)
        report["errors"] = errors
        failed = failed or bool(errors["wrong"])
        lines = [f"{errors['cases']} error cases, {len(errors['wrong'])} not as expected"]
        lines += [f"{entry['case']} ({entry['build']}): exit {entry['exit']}, {entry['errors'][:160]!r}" for entry in errors["wrong"]]
        notice("compare errors", lines)

    if "fuel" in sections:
        fuel = compare_fuel(before, after, [program for program in programs if program.group != "bugs"])
        report["fuel"] = fuel
        failed = failed or bool(fuel["different"])
        lines = [f"{fuel['programs']} programs, {len(fuel['different'])} differences, {len(fuel['skipped'])} skipped"]
        lines += [f"different: {entry['program']} at --fuel {entry['fuel']}, exit {entry['exit']}" for entry in fuel["different"]]
        lines += [f"skipped: {entry['program']}, {entry['why']}" for entry in fuel["skipped"]]
        notice("compare fuel", lines)

    if "timing" in sections:
        timed = [program for program in programs if program.group in ("bench", "micro")]
        timing = time_programs(before, after, timed, arguments.rounds, cycles_per_second)
        report["timing"] = timing
        notice("compare timing", timing_lines(timing))

    Path(arguments.out).write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"\nwrote {arguments.out}")
    return 1 if arguments.strict and failed else 0


if __name__ == "__main__":
    sys.exit(main())
