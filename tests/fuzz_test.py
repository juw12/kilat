"""Fuzz and differential tests for Kilat. Standard library only.

    python tests/fuzz_test.py                 full run with the default seed
    python tests/fuzz_test.py --seed 7        another seed
    python tests/fuzz_test.py --jobs 8        more worker processes side by side
    python tests/fuzz_test.py --selftest      prove that a crash and a hang are caught

What it does
    fuzz      broken copies of every .kilat file in the repo, plus token soup.
              The checker and the engine must never crash, hang or disagree.
    stress    very deep or very long programs (nesting, chains, runaway loops).
    diff      random arithmetic and logic, compared with a reference written
              in Python (64-bit overflow and division by zero must be refused).
    stats     mean, median, stddev against Python's statistics module.
    units     sums of lengths, masses and times against conversion factors.
    refusal   mixed dimensions (metres plus kilograms) must be refused.

Each case runs in a worker process. If the worker dies or stops making
progress, the parent records that case as a crash or a hang and carries on.
The exit code is 0 only when there are no failures.
"""

import argparse
import json
import math
import os
import random
import re
import shutil
import statistics
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
BINDING_FOLDER = REPO / "bindings" / "python"
CORPUS_FOLDERS = ["examples", "tests", "bench", "bugs"]

FUEL = 200_000
MEMORY_MB = 16
KNOWN_KINDS = {"", "compile", "runtime", "fuel", "memory"}
INT_MIN = -(2 ** 63)
INT_MAX = 2 ** 63 - 1

TOKEN = re.compile(
    r'[A-Za-z_][A-Za-z0-9_]*|\d+\.\d+|\d+|"(?:[^"\\\n]|\\.)*"|\s+|.',
    re.S,
)
EXTRA_TOKENS = [
    "{", "}", "(", ")", "[", "]", ",", ":", ";", ".", "..", "->", "=>", "=",
    "==", "!=", "<", ">", "<=", ">=", "+", "-", "*", "/", "%", "^", '"', "'",
    "\\", "//", "/*", "*/", "{{", "}}", "fn", "let", "var", "if", "else",
    "while", "for", "in", "return", "and", "or", "not", "true", "false",
    "print", "Int", "Float", "Bool", "String", "mm", "kg", "s", "0", "1",
    "9223372036854775807", "1e400", "0.0", "\n", "\t", "\r\n",
]
NOISE_CHARACTERS = (
    "abcxyz0189 \t\n\r{}()[]<>=!+-*/%^.,:;\"'\\#@$&|~`?_"
    "\u00e9\u4e2d\u200b\u00a0\U0001f600\x00\x01\x7f"
)

LENGTH = {
    "m": 1.0, "km": 1000.0, "cm": 0.01, "mm": 0.001, "um": 1e-6, "nm": 1e-9,
    "inch": 0.0254, "ft": 0.3048, "yd": 0.9144, "mile": 1609.344,
}
MASS = {
    "kg": 1.0, "g": 1e-3, "mg": 1e-6, "t": 1000.0,
    "lb": 0.45359237, "oz": 0.028349523125,
}
TIME = {
    "s": 1.0, "ms": 1e-3, "us": 1e-6, "ns": 1e-9,
    "min": 60.0, "h": 3600.0, "day": 86400.0,
}
DIMENSIONS = [LENGTH, MASS, TIME]


# ---------------------------------------------------------------- reference

class Refused(Exception):
    """The reference says the program must stop with this run-time error."""


class Skip(Exception):
    """The reference result is not finite, so the case is not compared."""


def checked_int(value):
    if value < INT_MIN or value > INT_MAX:
        raise Refused("overflow")
    return ("int", value)


def checked_float(value):
    if math.isinf(value) or math.isnan(value):
        raise Skip()
    return ("float", value)


def evaluate(node):
    """Reference evaluator. Values are (type name, value) pairs."""
    tag = node[0]
    if tag in ("int", "float", "bool"):
        return (tag, node[1])
    if tag == "neg":
        kind, value = evaluate(node[1])
        if kind == "int":
            return checked_int(-value)
        return checked_float(-value)
    if tag == "not":
        return ("bool", not evaluate(node[1])[1])
    if tag == "and":
        if not evaluate(node[1])[1]:
            return ("bool", False)
        return ("bool", evaluate(node[2])[1])
    if tag == "or":
        if evaluate(node[1])[1]:
            return ("bool", True)
        return ("bool", evaluate(node[2])[1])
    operator = node[1]
    left_kind, left = evaluate(node[2])
    right_kind, right = evaluate(node[3])
    if tag == "compare":
        if left_kind != right_kind:
            left = float(left)
            right = float(right)
        result = {
            "<": left < right, "<=": left <= right,
            ">": left > right, ">=": left >= right,
            "==": left == right, "!=": left != right,
        }[operator]
        return ("bool", result)
    both_int = left_kind == "int" and right_kind == "int"
    if operator == "%":
        return checked_int(left % right)
    if operator == "/":
        if float(right) == 0.0:
            raise Refused("division by zero")
        return checked_float(float(left) / float(right))
    if both_int:
        if operator == "+":
            return checked_int(left + right)
        if operator == "-":
            return checked_int(left - right)
        return checked_int(left * right)
    left = float(left)
    right = float(right)
    if operator == "+":
        return checked_float(left + right)
    if operator == "-":
        return checked_float(left - right)
    return checked_float(left * right)


def int_literal(rng):
    choice = rng.random()
    if choice < 0.70:
        return ("int", rng.randint(-20, 20))
    if choice < 0.90:
        return ("int", rng.randint(-1_000_000, 1_000_000))
    if choice < 0.96:
        return ("int", rng.choice([2 ** 31, 2 ** 32, 3037000500, 2 ** 40]) + rng.randint(-3, 3))
    return ("int", rng.choice([2 ** 62, INT_MAX, INT_MAX - 1, -(2 ** 62), -INT_MAX]))


def float_literal(rng):
    choice = rng.random()
    if choice < 0.10:
        return ("float", 0.0)
    if choice < 0.70:
        return ("float", round(rng.uniform(-10, 10), 3))
    return ("float", round(rng.uniform(-1000, 1000), 3))


def make_int(rng, depth):
    if depth <= 0 or rng.random() < 0.25:
        return int_literal(rng)
    choice = rng.random()
    if choice < 0.10:
        return ("neg", make_int(rng, depth - 1))
    if choice < 0.25:
        return ("bin", "%", make_int(rng, depth - 1), ("int", rng.randint(1, 97)))
    operator = rng.choice(["+", "-", "*"])
    return ("bin", operator, make_int(rng, depth - 1), make_int(rng, depth - 1))


def make_number(rng, depth):
    if depth <= 0 or rng.random() < 0.20:
        if rng.random() < 0.5:
            return int_literal(rng)
        return float_literal(rng)
    choice = rng.random()
    if choice < 0.30:
        return make_int(rng, depth)
    if choice < 0.38:
        return ("neg", make_number(rng, depth - 1))
    operator = rng.choice(["+", "-", "*", "/"])
    return ("bin", operator, make_number(rng, depth - 1), make_number(rng, depth - 1))


def make_bool(rng, depth):
    if depth <= 0 or rng.random() < 0.10:
        return ("bool", rng.random() < 0.5)
    choice = rng.random()
    if choice < 0.50:
        operator = rng.choice(["<", "<=", ">", ">=", "==", "!="])
        return ("compare", operator, make_number(rng, depth - 1), make_number(rng, depth - 1))
    if choice < 0.60:
        operator = rng.choice(["==", "!="])
        return ("compare", operator, make_bool(rng, depth - 1), make_bool(rng, depth - 1))
    if choice < 0.72:
        return ("not", make_bool(rng, depth - 1))
    tag = rng.choice(["and", "or"])
    return (tag, make_bool(rng, depth - 1), make_bool(rng, depth - 1))


def literal_text(node):
    tag, value = node
    if tag == "bool":
        return "true" if value else "false"
    if tag == "int":
        return str(value) if value >= 0 else "(-%d)" % -value
    body = "%.3f" % abs(value)
    return body if value >= 0 and not str(value).startswith("-") else "(-%s)" % body


def render(node, names):
    """Kilat source for a tree. With a names list, literals become variables."""
    tag = node[0]
    if tag in ("int", "float", "bool"):
        if names is None:
            return literal_text(node)
        names.append(literal_text(node))
        return "v%d" % (len(names) - 1)
    if tag == "neg":
        return "(-(%s))" % render(node[1], names)
    if tag == "not":
        return "(not %s)" % render(node[1], names)
    if tag in ("and", "or"):
        return "(%s %s %s)" % (render(node[1], names), tag, render(node[2], names))
    return "(%s %s %s)" % (render(node[2], names), node[1], render(node[3], names))


def diff_case(rng):
    depth = rng.randint(1, 5)
    family = rng.random()
    if family < 0.35:
        tree = make_int(rng, depth)
    elif family < 0.70:
        tree = make_number(rng, depth)
    else:
        tree = make_bool(rng, depth)
    if rng.random() < 0.5:
        source = "print(%s)" % render(tree, None)
    else:
        names = []
        body = render(tree, names)
        lines = ["let v%d = %s" % (index, text) for index, text in enumerate(names)]
        lines.append("print(%s)" % body)
        source = "\n".join(lines)
    try:
        kind, value = evaluate(tree)
        expected = {"type": kind, "value": value}
    except Refused as refusal:
        expected = {"type": "error", "value": str(refusal)}
    except Skip:
        expected = {"type": "skip", "value": None}
    return {"cat": "diff", "source": source, "expected": expected}


def number_text(value):
    if isinstance(value, int):
        return str(value) if value >= 0 else "(-%d)" % -value
    return "%.3f" % value if value >= 0 else "(-%.3f)" % -value


def stats_case(rng):
    count = rng.randint(2, 12)
    if rng.random() < 0.3:
        values = [rng.randint(-500, 500) for _ in range(count)]
    else:
        values = [round(rng.uniform(-500, 500), 3) for _ in range(count)]
    name = rng.choice(["mean", "median", "stddev"])
    reference = {
        "mean": statistics.fmean,
        "median": statistics.median,
        "stddev": statistics.stdev,
    }[name]
    expected = float(reference([float(value) for value in values]))
    scale = max(1.0, max(abs(float(value)) for value in values))
    source = "print(%s([%s]))" % (name, ", ".join(number_text(value) for value in values))
    return {
        "cat": "stats",
        "source": source,
        "expected": {"type": "float", "value": expected, "scale": scale},
    }


def units_case(rng):
    table = rng.choice(DIMENSIONS)
    names = list(table)
    total = 0.0
    parts = []
    used = []
    for _ in range(rng.randint(1, 4)):
        unit = rng.choice(names)
        if rng.random() < 0.4:
            amount = rng.randint(1, 500)
            parts.append("%d %s" % (amount, unit))
        else:
            amount = round(rng.uniform(0.001, 500), 3)
            parts.append("%.3f %s" % (amount, unit))
        total += amount * table[unit]
        used.append(unit)
    target = rng.choice(names)
    used.append(target)
    source = "print((%s) in %s)" % (" + ".join(parts), target)
    return {
        "cat": "units",
        "source": source,
        "units": used,
        "expected": {"type": "float", "value": total / table[target]},
    }


def refusal_case(rng):
    first, second = rng.sample(range(len(DIMENSIONS)), 2)
    unit_a = rng.choice(list(DIMENSIONS[first]))
    unit_b = rng.choice(list(DIMENSIONS[second]))
    if rng.random() < 0.5:
        source = "print((%d %s + %d %s) in %s)" % (
            rng.randint(1, 9), unit_a, rng.randint(1, 9), unit_b, unit_a,
        )
    else:
        source = "print((%d %s) in %s)" % (rng.randint(1, 9), unit_a, unit_b)
    return {"cat": "refusal", "source": source, "units": [unit_a, unit_b]}


# --------------------------------------------------------------------- fuzz

def read_corpus():
    corpus = []
    for folder in CORPUS_FOLDERS:
        for path in sorted((REPO / folder).rglob("*.kilat")):
            corpus.append(path.read_text(encoding="utf-8", errors="replace"))
    return corpus


def mutate(rng, source, corpus, vocabulary):
    tokens = TOKEN.findall(source)
    for _ in range(rng.randint(1, 4)):
        if not tokens:
            tokens = [rng.choice(vocabulary)]
        position = rng.randrange(len(tokens))
        action = rng.choice(
            ["delete", "duplicate", "swap", "replace", "insert", "noise", "truncate", "splice"]
        )
        if action == "delete":
            del tokens[position]
        elif action == "duplicate":
            tokens.insert(position, tokens[position])
        elif action == "swap":
            other = rng.randrange(len(tokens))
            tokens[position], tokens[other] = tokens[other], tokens[position]
        elif action == "replace":
            tokens[position] = rng.choice(vocabulary)
        elif action == "insert":
            tokens.insert(position, rng.choice(vocabulary))
        elif action == "noise":
            noise = "".join(rng.choice(NOISE_CHARACTERS) for _ in range(rng.randint(1, 8)))
            tokens.insert(position, noise)
        elif action == "truncate":
            joined = "".join(tokens)
            tokens = TOKEN.findall(joined[: rng.randrange(len(joined) + 1)])
        else:
            donor = TOKEN.findall(rng.choice(corpus))
            if donor:
                start = rng.randrange(len(donor))
                tokens[position:position] = donor[start : start + rng.randint(1, 30)]
    return "".join(tokens)


def soup(rng, vocabulary):
    pieces = []
    for _ in range(rng.randint(1, 120)):
        pieces.append(rng.choice(vocabulary))
        pieces.append(rng.choice([" ", " ", "", "\n"]))
    return "".join(pieces)


def stress_sources():
    deep = 3000
    return [
        "",
        "   \n\t\r\n",
        "\ufeffprint(1)",
        "print(" + "(" * deep + "1" + ")" * deep + ")",
        "print(" + "[" * deep + "1" + "]" * deep + ")",
        "print(" + '["a": ' * 2000 + "1" + "]" * 2000 + ")",
        "print(" + " + ".join(["1"] * 5000) + ")",
        "print(" + " + ".join(['"a"'] * 5000) + ")",
        "print(" + " < ".join(["1"] * 3000) + ")",
        "print(" + "- " * 5000 + "1)",
        "print(" + "-(" * deep + "1" + ")" * deep + ")",
        "print(" + "not " * 5000 + "true)",
        "print(1" + " ^ 1" * deep + ")",
        "{" * deep + "}" * deep,
        "if true { " * 2000 + "print(1) " + "} " * 2000,
        "if false { }" + " else if false { }" * 3000 + " else { print(1) }",
        "fn f(x: Int) -> Int { return x }\nprint(" + "f(" * 2000 + "1" + ")" * 2000 + ")",
        "let xs = [1]\nprint(xs" + "[0]" * deep + ")",
        "\n".join("let v%d = %d" % (index, index) for index in range(3000)) + "\nprint(v2999)",
        'print("' + "a" * 200000 + '")',
        'print("' + "{" * 500 + "1" + "}" * 500 + '")',
        'print("' + "{1}" * 5000 + '")',
        '"' + "a" * 10000,
        "x" * 100000,
        "print(" + "1" * 5000 + ")",
        "print(1" + "0" * 400 + ".0)",
        "print(0." + "0" * 400 + "1)",
        "// " + "c" * 100000,
        "fn f(n: Int) -> Int { return f(n + 1) }\nprint(f(0))",
        "fn f(n: Int) -> Int { return 1 + f(n + 1) }\nprint(f(0))",
        'var s = "a"\nwhile true { s = s + s }',
        "var xs = [1]\nwhile true { xs = xs + xs }",
        "while true { }",
        "var n = 0\nwhile true { n = n + 1 }",
        "for i in 0..9223372036854775807 { }",
        "print(9223372036854775807 + 1)",
        "print((-9223372036854775807) - 2)",
        "print(1 / 0)",
        "print(1 % 0)",
        "print(2 ^ 4000)",
        "print(2.0 ^ 4000.0)",
        "print([1, 2, 3][99])",
        "print([1, 2, 3][-1])",
        'print(["a": 1]["b"])',
        "print(mean([]))",
        "print(stddev([1.0]))",
    ]


def build_cases(rng, arguments):
    corpus = read_corpus()
    if not corpus:
        raise SystemExit("no .kilat files found under " + str(REPO))
    vocabulary = sorted(
        {token for text in corpus for token in TOKEN.findall(text) if token.strip()}
    ) + EXTRA_TOKENS
    cases = []
    if not arguments.selftest:
        for source in stress_sources():
            cases.append({"cat": "stress", "source": source, "twice": True})
    for text in corpus:
        cases.append({"cat": "fuzz", "source": text, "twice": True})
    for index in range(arguments.fuzz):
        if index % 8 == 7:
            source = soup(rng, vocabulary)
        else:
            source = mutate(rng, rng.choice(corpus), corpus, vocabulary)
        # Every fifth broken program is run twice to see that the result repeats.
        cases.append({"cat": "fuzz", "source": source, "twice": index % 5 == 0})
    for _ in range(arguments.diff):
        cases.append(diff_case(rng))
    for _ in range(arguments.stats):
        cases.append(stats_case(rng))
    for _ in range(arguments.units):
        cases.append(units_case(rng))
    for _ in range(arguments.units // 2):
        cases.append(refusal_case(rng))
    if arguments.selftest:
        cases.append({"cat": "selftest_crash", "source": ""})
        cases.append({"cat": "selftest_hang", "source": ""})
        cases.append({"cat": "stress", "source": "print(1)"})
    return cases


# ------------------------------------------------------------------- worker

def looks_like_whole_number(text):
    return re.fullmatch(r"-?\d+", text) is not None


def compare_value(expected, result):
    """Return a failure text, or None when the output matches the reference."""
    wanted = expected["type"]
    if wanted == "error":
        if result.ok or result.kind != "runtime" or expected["value"] not in (result.error or ""):
            return "expected run-time error '%s', got kind '%s': %s" % (
                expected["value"], result.kind, (result.output or result.error or "")[:120],
            )
        return None
    if not result.ok:
        return "expected a value, got %s error: %s" % (result.kind, (result.error or "")[:160])
    shown = (result.output or "").strip()
    if wanted == "bool":
        want_text = "true" if expected["value"] else "false"
        return None if shown == want_text else "expected %s, got %s" % (want_text, shown[:80])
    if wanted == "int":
        return None if shown == str(expected["value"]) else "expected %d, got %s" % (
            expected["value"], shown[:80],
        )
    if looks_like_whole_number(shown):
        return "expected a float, but the output is written as a whole number: " + shown[:80]
    try:
        got = float(shown)
    except ValueError:
        return "expected a float, got " + shown[:80]
    want = expected["value"]
    slack = expected.get("relative", 1e-11) * abs(want) + 1e-12 * expected.get("scale", 0.0)
    if abs(got - want) <= slack:
        return None
    return "expected %r, got %s" % (want, shown[:80])


def run_case(kilat, case, unit_known):
    category = case["cat"]
    source = case["source"]
    if category == "selftest_crash":
        os._exit(77)
    if category == "selftest_hang":
        time.sleep(3600)
    for unit in case.get("units", []):
        if unit not in unit_known:
            unit_known[unit] = kilat.check("print(1 %s)" % unit).ok
        if not unit_known[unit]:
            return {"skip": "unit %s is not built in" % unit}
    checked = kilat.check(source)
    first = kilat.run(source, fuel=FUEL, mem_mb=MEMORY_MB, seed=1)
    report = {"kind": first.kind, "check_ok": bool(checked.ok)}
    problems = []
    for name, result in (("check", checked), ("run", first)):
        if result.kind not in KNOWN_KINDS:
            problems.append("%s gave an unknown kind %r" % (name, result.kind))
        if bool(result.ok) != (result.kind == ""):
            problems.append("%s: ok=%r does not match kind %r" % (name, result.ok, result.kind))
    if checked.kind not in ("", "compile"):
        problems.append("check gave kind %r" % checked.kind)
    if checked.ok and first.kind == "compile":
        problems.append("check passed but run refused to compile: " + (first.error or "")[:160])
    if not checked.ok and first.kind != "compile":
        problems.append("check failed but run started (kind %r)" % first.kind)
    if category in ("fuzz", "stress"):
        if case.get("twice"):
            second = kilat.run(source, fuel=FUEL, mem_mb=MEMORY_MB, seed=1)
            if (first.ok, first.kind, first.output) != (second.ok, second.kind, second.output):
                problems.append("two runs of the same program differ")
    elif category == "refusal":
        if checked.ok:
            problems.append("mixed dimensions were accepted")
    else:
        expected = case["expected"]
        if expected["type"] == "skip":
            report["skip"] = "reference result is not finite"
        else:
            if not checked.ok:
                problems.append("check refused a valid program: " + (checked.error or "")[:160])
            if category == "units":
                expected = dict(expected, relative=1e-9)
            mismatch = compare_value(expected, first)
            if mismatch:
                problems.append(mismatch)
            report["expected"] = expected["type"]
    if problems:
        report["fail"] = "; ".join(problems)
    return report


def worker(folder, start):
    sys.path.insert(0, str(BINDING_FOLDER))
    import kilat

    cases = json.loads((folder / "cases.json").read_text(encoding="utf-8"))
    unit_known = {}
    with open(folder / "progress.txt", "w", encoding="utf-8") as progress, open(
        folder / "results.jsonl", "a", encoding="utf-8"
    ) as results:
        for index in range(start, len(cases)):
            progress.seek(0)
            progress.write("%10d" % index)
            progress.flush()
            began = time.perf_counter()
            try:
                report = run_case(kilat, cases[index], unit_known)
            except Exception as error:
                report = {"fail": "Python exception: %s: %s" % (type(error).__name__, error)}
            report["i"] = index
            report["seconds"] = round(time.perf_counter() - began, 4)
            results.write(json.dumps(report) + "\n")
            results.flush()
    return 0


# ------------------------------------------------------------------- parent

def read_progress(path):
    try:
        return int(path.read_text(encoding="utf-8").strip())
    except (OSError, ValueError):
        return None


def read_results(path):
    reports = {}
    if path.exists():
        for line in path.read_text(encoding="utf-8").splitlines():
            try:
                report = json.loads(line)
            except ValueError:
                continue
            reports[report["i"]] = report
    return reports


def supervise(folder, total, hang_seconds):
    """Run workers until every case has a result. Returns (reports, restarts)."""
    progress_path = folder / "progress.txt"
    results_path = folder / "results.jsonl"
    stopped = {}
    start = 0
    restarts = 0
    while start < total:
        with open(folder / "worker_errors.txt", "a", encoding="utf-8") as errors:
            process = subprocess.Popen(
                [sys.executable, str(Path(__file__).resolve()), "--worker", str(folder),
                 "--start", str(start)],
                stdout=subprocess.DEVNULL,
                stderr=errors,
                stdin=subprocess.DEVNULL,
            )
            seen = None
            changed = time.monotonic()
            hung = False
            while process.poll() is None:
                now = read_progress(progress_path)
                if now != seen:
                    seen = now
                    changed = time.monotonic()
                elif time.monotonic() - changed > hang_seconds:
                    hung = True
                    process.kill()
                    process.wait()
                    break
                time.sleep(0.05)
        reports = read_results(results_path)
        current = read_progress(progress_path)
        if current is None or current < start:
            current = start
        if not hung and process.returncode == 0 and all(i in reports for i in range(start, total)):
            break
        if current not in reports:
            if hung:
                stopped[current] = "hang: no progress for %s s" % hang_seconds
            else:
                stopped[current] = "crash: the worker exited with code %s (0x%08X)" % (
                    process.returncode, process.returncode & 0xFFFFFFFF,
                )
        start = current + 1
        restarts += 1
    reports = read_results(results_path)
    for index, reason in stopped.items():
        reports[index] = {"i": index, "fail": reason, "seconds": 0.0}
    return reports, restarts


def run_shards(cases, jobs, hang_seconds):
    """Split the cases over several supervised workers. Returns (reports, restarts, errors)."""
    jobs = max(1, min(jobs, len(cases)))
    shards = [list(range(first, len(cases), jobs)) for first in range(jobs)]
    outcomes = [None] * jobs

    def one_shard(number):
        folder = Path(tempfile.mkdtemp(prefix="kilat_fuzz_"))
        try:
            chosen = [cases[index] for index in shards[number]]
            (folder / "cases.json").write_text(json.dumps(chosen), encoding="utf-8")
            local_reports, restarts = supervise(folder, len(chosen), hang_seconds)
            errors = (folder / "worker_errors.txt").read_text(encoding="utf-8", errors="replace")
            outcomes[number] = (local_reports, restarts, errors)
        finally:
            shutil.rmtree(folder, ignore_errors=True)

    threads = [threading.Thread(target=one_shard, args=(number,)) for number in range(jobs)]
    for thread in threads:
        thread.start()
    for thread in threads:
        thread.join()
    reports = {}
    restarts = 0
    errors_text = ""
    for number, outcome in enumerate(outcomes):
        if outcome is None:
            raise SystemExit("shard %d did not finish" % number)
        local_reports, shard_restarts, errors = outcome
        restarts += shard_restarts
        errors_text += errors
        for local_index, report in local_reports.items():
            reports[shards[number][local_index]] = report
    return reports, restarts, errors_text


def summarise(cases, reports, restarts, arguments, elapsed):
    by_category = {}
    kinds = {}
    failures = []
    skipped = 0
    refused_as_expected = 0
    longest = 0.0
    for index, case in enumerate(cases):
        category = case["cat"]
        by_category[category] = by_category.get(category, 0) + 1
        report = reports.get(index)
        if report is None:
            report = {"fail": "no result was recorded"}
        longest = max(longest, report.get("seconds", 0.0))
        if "kind" in report:
            label = report["kind"] or "ok"
            kinds[label] = kinds.get(label, 0) + 1
        if "skip" in report:
            skipped += 1
        if case.get("expected", {}).get("type") == "error" and "fail" not in report:
            refused_as_expected += 1
        if "fail" in report:
            failures.append(
                {
                    "index": index,
                    "category": category,
                    "reason": report["fail"],
                    "source": case["source"][:1500],
                }
            )
    return {
        "seed": arguments.seed,
        "cases": len(cases),
        "by_category": by_category,
        "run_outcomes": kinds,
        "diff_cases_refused_as_expected": refused_as_expected,
        "skipped": skipped,
        "failures": len(failures),
        "failure_list": failures[:60],
        "worker_restarts": restarts,
        "longest_case_seconds": longest,
        "total_seconds": round(elapsed, 1),
        "fuel_per_run": FUEL,
        "memory_mb_per_run": MEMORY_MB,
    }


def main():
    parser = argparse.ArgumentParser(description="Fuzz and differential tests for Kilat.")
    parser.add_argument("--seed", type=int, default=20261112)
    parser.add_argument("--fuzz", type=int, default=6000)
    parser.add_argument("--diff", type=int, default=4000)
    parser.add_argument("--stats", type=int, default=600)
    parser.add_argument("--units", type=int, default=600)
    parser.add_argument("--hang-seconds", type=float, default=60.0)
    parser.add_argument("--jobs", type=int, default=4, help="worker processes side by side")
    parser.add_argument("--out", help="write the summary to this JSON file")
    parser.add_argument("--selftest", action="store_true")
    parser.add_argument("--worker", help=argparse.SUPPRESS)
    parser.add_argument("--start", type=int, default=0, help=argparse.SUPPRESS)
    arguments = parser.parse_args()
    if arguments.worker:
        return worker(Path(arguments.worker), arguments.start)

    if arguments.selftest:
        arguments.fuzz = 20
        arguments.diff = 20
        arguments.stats = 5
        arguments.units = 4
        arguments.hang_seconds = 5.0
        arguments.jobs = 1
    began = time.monotonic()
    cases = build_cases(random.Random(arguments.seed), arguments)
    reports, restarts, errors_text = run_shards(cases, arguments.jobs, arguments.hang_seconds)
    summary = summarise(cases, reports, restarts, arguments, time.monotonic() - began)
    summary["jobs"] = arguments.jobs
    if errors_text.strip():
        summary["worker_error_tail"] = errors_text[-600:]
    if arguments.selftest:
        found = {item["category"]: item["reason"] for item in summary["failure_list"]}
        summary["selftest_passed"] = (
            found.get("selftest_crash", "").startswith("crash")
            and found.get("selftest_hang", "").startswith("hang")
            and "fail" not in reports.get(len(cases) - 1, {"fail": "missing"})
        )
    if arguments.out:
        Path(arguments.out).write_text(json.dumps(summary, indent=2), encoding="utf-8")
    brief = {key: value for key, value in summary.items() if key != "failure_list"}
    print(json.dumps(brief, indent=2))
    for item in summary["failure_list"][:10]:
        print("FAIL #%d (%s): %s" % (item["index"], item["category"], item["reason"]))
        print("    " + item["source"][:200].replace("\n", "\\n"))
    if arguments.selftest:
        return 0 if summary["selftest_passed"] else 1
    return 0 if summary["failures"] == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
