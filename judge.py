"""Measures what the README says about Kilat. Standard library only.

    python judge.py            every section (several minutes; opens a headless browser)
    python judge.py --quick    fewer timing runs, and no browser sections

It writes judge_report.json (everything measured) and RESULTS.md (the tables).
The exit code is 0 when every correctness check passes. Speed is reported, not judged.
"""

import argparse
import importlib.util
import json
import math
import os
import platform
import py_compile
import statistics
import struct
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent
WORK = ROOT / "judge_work"
WINDOWS = sys.platform == "win32"
SPEED_TARGET = 5.0  # geometric mean over CPython, set before anything was measured

if WINDOWS:
    import ctypes
    import ctypes.wintypes as wintypes
    import winreg

    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel32.QueryProcessCycleTime.argtypes = [wintypes.HANDLE, ctypes.POINTER(ctypes.c_ulonglong)]
    kernel32.QueryProcessCycleTime.restype = wintypes.BOOL
    kernel32.GetCurrentProcess.restype = wintypes.HANDLE
    kernel32.GetProcessAffinityMask.argtypes = [
        wintypes.HANDLE,
        ctypes.POINTER(ctypes.c_size_t),
        ctypes.POINTER(ctypes.c_size_t),
    ]
    kernel32.SetProcessAffinityMask.argtypes = [wintypes.HANDLE, ctypes.c_size_t]
    kernel32.GetLogicalProcessorInformationEx.argtypes = [
        ctypes.c_int,
        ctypes.c_void_p,
        ctypes.POINTER(wintypes.DWORD),
    ]
else:
    import resource


def kilat_program():
    return ROOT / "target" / "release" / ("kilat.exe" if WINDOWS else "kilat")


def kilat_library():
    name = "kilat_core.dll" if WINDOWS else ("libkilat_core.dylib" if sys.platform == "darwin" else "libkilat_core.so")
    return ROOT / "target" / "release" / name


def processor():
    """(name, cycles per second) of the processor; the second is None away from Windows."""
    if not WINDOWS:
        return platform.processor() or platform.machine(), None
    with winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE, r"HARDWARE\DESCRIPTION\System\CentralProcessor\0") as key:
        name = winreg.QueryValueEx(key, "ProcessorNameString")[0].strip()
        megahertz = winreg.QueryValueEx(key, "~MHz")[0]
    return name, megahertz * 1e6


def fastest_cores_mask():
    """Bit mask of the logical processors on the fastest kind of core; 0 when unknown."""
    if not WINDOWS:
        return 0
    relation_core = 0
    needed = wintypes.DWORD(0)
    kernel32.GetLogicalProcessorInformationEx(relation_core, None, ctypes.byref(needed))
    buffer = ctypes.create_string_buffer(needed.value)
    if not kernel32.GetLogicalProcessorInformationEx(relation_core, buffer, ctypes.byref(needed)):
        return 0
    cores = []
    offset = 0
    while offset < needed.value:
        _relationship, size = struct.unpack_from("<II", buffer, offset)
        efficiency = struct.unpack_from("<B", buffer, offset + 9)[0]
        mask = struct.unpack_from("<Q", buffer, offset + 32)[0]
        cores.append((efficiency, mask))
        offset += size
    best = max(efficiency for efficiency, _ in cores)
    combined = 0
    for efficiency, mask in cores:
        if efficiency == best:
            combined |= mask
    return combined


class Pinned:
    """Keeps this process, and the ones it starts, on the fastest cores for a while."""

    def __enter__(self):
        self.before = 0
        self.mask = fastest_cores_mask()
        if self.mask:
            process_mask = ctypes.c_size_t(0)
            system_mask = ctypes.c_size_t(0)
            process = kernel32.GetCurrentProcess()
            if kernel32.GetProcessAffinityMask(process, ctypes.byref(process_mask), ctypes.byref(system_mask)):
                self.before = process_mask.value
                self.mask &= system_mask.value
                if not self.mask or not kernel32.SetProcessAffinityMask(process, self.mask):
                    self.mask = 0
        return self

    def __exit__(self, *_):
        if self.mask and self.before:
            kernel32.SetProcessAffinityMask(kernel32.GetCurrentProcess(), self.before)


def run_measured(command, cwd, cycles_per_second):
    """Runs a command to its end; returns (CPU seconds, wall seconds, printed text, exit code)."""
    if not WINDOWS:
        before = resource.getrusage(resource.RUSAGE_CHILDREN)
    began = time.perf_counter()
    process = subprocess.Popen(command, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    printed, _errors = process.communicate()
    wall = time.perf_counter() - began
    if WINDOWS:
        cycles = ctypes.c_ulonglong(0)
        if not kernel32.QueryProcessCycleTime(int(process._handle), ctypes.byref(cycles)):
            raise OSError(ctypes.get_last_error(), "QueryProcessCycleTime failed")
        cpu = cycles.value / cycles_per_second
    else:
        after = resource.getrusage(resource.RUSAGE_CHILDREN)
        cpu = (after.ru_utime - before.ru_utime) + (after.ru_stime - before.ru_stime)
    return cpu, wall, printed.decode("utf-8", "replace").replace("\r\n", "\n"), process.returncode


def run_quiet(command, cwd=ROOT, timeout=120):
    """Runs a command; returns (exit code, printed text, error text)."""
    finished = subprocess.run(command, cwd=cwd, capture_output=True, timeout=timeout)
    decode = lambda raw: raw.decode("utf-8", "replace").replace("\r\n", "\n")
    return finished.returncode, decode(finished.stdout), decode(finished.stderr)


def timed(command, cwd, runs, cycles_per_second):
    cpu_times = []
    wall_times = []
    printed = None
    for _ in range(runs):
        cpu, wall, text, code = run_measured(command, cwd, cycles_per_second)
        if code != 0:
            raise RuntimeError(f"{command} ended with code {code}")
        cpu_times.append(cpu)
        wall_times.append(wall)
        printed = text
    return {
        "cpu_best": min(cpu_times),
        "cpu_median": statistics.median(cpu_times),
        "wall_best": min(wall_times),
        "output": printed,
    }


def geometric_mean(values):
    return math.exp(sum(math.log(value) for value in values) / len(values)) if values else None


def measure_speed(runs, cycles_per_second):
    """Kilat against the Python that runs this script, on the frozen programs in bench/."""
    WORK.mkdir(exist_ok=True)
    (WORK / "empty.kilat").write_text("", encoding="utf-8")
    (WORK / "empty.py").write_text("", encoding="utf-8")
    kilat = str(kilat_program())
    with Pinned() as pinned:
        start_kilat = timed([kilat, "run", "empty.kilat"], WORK, runs * 2, cycles_per_second)
        start_python = timed([sys.executable, "empty.py"], WORK, runs * 2, cycles_per_second)
        rows = []
        for source in sorted((ROOT / "bench").glob("*.kilat")):
            ours = timed([kilat, "run", source.name], source.parent, runs, cycles_per_second)
            theirs = timed([sys.executable, source.with_suffix(".py").name], source.parent, runs, cycles_per_second)
            net_ours = ours["cpu_best"] - start_kilat["cpu_best"]
            net_theirs = theirs["cpu_best"] - start_python["cpu_best"]
            rows.append({
                "name": source.stem,
                "same_output": ours["output"] == theirs["output"],
                "kilat_cpu": ours["cpu_best"],
                "kilat_cpu_median": ours["cpu_median"],
                "python_cpu": theirs["cpu_best"],
                "python_cpu_median": theirs["cpu_median"],
                "kilat_wall": ours["wall_best"],
                "python_wall": theirs["wall_best"],
                "ratio": theirs["cpu_best"] / ours["cpu_best"],
                "ratio_net": net_theirs / net_ours if net_ours > 0.001 and net_theirs > 0 else None,
            })
    net = [row["ratio_net"] for row in rows if row["ratio_net"]]
    return {
        "runs": runs,
        "pinned_to_fastest_cores": bool(pinned.mask),
        "cpu_time_from": "QueryProcessCycleTime" if WINDOWS else "getrusage",
        "startup": {
            "kilat_cpu": start_kilat["cpu_best"],
            "python_cpu": start_python["cpu_best"],
            "kilat_wall": start_kilat["wall_best"],
            "python_wall": start_python["wall_best"],
        },
        "benchmarks": rows,
        "geometric_mean": geometric_mean([row["ratio"] for row in rows]),
        "geometric_mean_net": geometric_mean(net),
        "faster": [row["name"] for row in rows if row["ratio_net"] and row["ratio_net"] > 1.05],
        "slower": [row["name"] for row in rows if row["ratio_net"] and row["ratio_net"] < 0.95],
        "outputs_match": all(row["same_output"] for row in rows),
    }


def pe_imports(path):
    """Names of the libraries a Windows program or DLL needs, from its import table."""
    data = path.read_bytes()
    header = struct.unpack_from("<I", data, 0x3C)[0]
    if data[:2] != b"MZ" or data[header : header + 4] != b"PE\0\0":
        return None
    section_count = struct.unpack_from("<H", data, header + 6)[0]
    optional_size = struct.unpack_from("<H", data, header + 20)[0]
    optional = header + 24
    directories = optional + (112 if struct.unpack_from("<H", data, optional)[0] == 0x20B else 96)
    import_address = struct.unpack_from("<I", data, directories + 8)[0]
    sections = []
    for index in range(section_count):
        entry = optional + optional_size + 40 * index
        virtual_size, address, raw_size, raw_pointer = struct.unpack_from("<IIII", data, entry + 8)
        sections.append((address, max(virtual_size, raw_size), raw_pointer))

    def file_offset(address):
        for start, size, pointer in sections:
            if start <= address < start + size:
                return address - start + pointer
        raise ValueError("address outside every section")

    names = set()
    position = file_offset(import_address) if import_address else None
    while position is not None:
        name_address = struct.unpack_from("<I", data, position + 12)[0]
        if name_address == 0:
            break
        start = file_offset(name_address)
        names.add(data[start : data.index(b"\0", start)].decode("ascii").lower())
        position += 20
    return sorted(names)


def part_of_windows(library):
    if library.startswith(("api-ms-win-", "ext-ms-")):
        return True
    return (Path(os.environ.get("SystemRoot", r"C:\Windows")) / "System32" / library).is_file()


def wasm_import_count(path):
    data = path.read_bytes()

    def number(position):
        value = 0
        shift = 0
        while True:
            byte = data[position]
            position += 1
            value |= (byte & 0x7F) << shift
            shift += 7
            if byte < 0x80:
                return value, position

    position = 8
    while position < len(data):
        section = data[position]
        size, position = number(position + 1)
        if section == 2:
            return number(position)[0]
        position += size
    return 0


def measure_sizes():
    files = {
        "kilat program": kilat_program(),
        "kilat library": kilat_library(),
        "docs/kilat.wasm": ROOT / "docs" / "kilat.wasm",
        "docs/kilat.js": ROOT / "docs" / "kilat.js",
        "main.js (Cairn plugin)": ROOT / "main.js",
    }
    sizes = {name: path.stat().st_size for name, path in files.items() if path.is_file()}
    needs = {}
    if WINDOWS:
        for name in ("kilat program", "kilat library"):
            if files[name].is_file():
                needs[name] = pe_imports(files[name])
    wasm = ROOT / "docs" / "kilat.wasm"
    return {
        "bytes": sizes,
        "needs": needs,
        "needs_only_windows": all(part_of_windows(library) for libraries in needs.values() for library in libraries),
        "wasm_imports": wasm_import_count(wasm) if wasm.is_file() else None,
    }


def measure_bugs():
    """Each bug in bugs/: does Kilat refuse it, and what do Python and mypy do with its twin?"""
    folder = ROOT / "bugs"
    kilat = str(kilat_program())
    have_mypy = importlib.util.find_spec("mypy") is not None
    rows = []
    for entry in json.loads((folder / "manifest.json").read_text(encoding="utf-8")):
        name = entry["name"]
        code, _, errors = run_quiet([kilat, "check", f"{name}.kilat"], folder)
        fixed_code, _, _ = run_quiet([kilat, "run", f"{name}.fixed.kilat"], folder)
        try:
            py_compile.compile(str(folder / f"{name}.py"), cfile=str(WORK / "twin.pyc"), doraise=True)
            compiles = True
        except py_compile.PyCompileError:
            compiles = False
        twin_code, _, _ = run_quiet([sys.executable, f"{name}.py"], folder, timeout=60)
        mypy = None
        if have_mypy:
            mypy_code, _, _ = run_quiet(
                [sys.executable, "-m", "mypy", "--strict", "--no-error-summary", "--no-incremental", f"{name}.py"],
                folder,
                timeout=300,
            )
            mypy = mypy_code == 1
        rows.append({
            "name": name,
            "category": entry["category"],
            "kilat_refuses": code != 0 and entry["expected_error"] in errors,
            "fixed_runs": fixed_code == 0,
            "python_compiles": compiles,
            "python_crashes_when_run": twin_code != 0,
            "mypy_strict_catches": mypy,
        })
    categories = {}
    for row in rows:
        group = categories.setdefault(row["category"], {"bugs": 0, "kilat": 0, "mypy": 0, "python_before_running": 0})
        group["bugs"] += 1
        group["kilat"] += row["kilat_refuses"]
        group["mypy"] += bool(row["mypy_strict_catches"])
        group["python_before_running"] += not row["python_compiles"]
    return {
        "bugs": rows,
        "categories": categories,
        "total": len(rows),
        "kilat_refuses": sum(row["kilat_refuses"] for row in rows),
        "fixed_run": sum(row["fixed_runs"] for row in rows),
        "python_refuses_before_running": sum(not row["python_compiles"] for row in rows),
        "python_crashes_when_run": sum(row["python_crashes_when_run"] for row in rows),
        "python_runs_to_a_wrong_answer": sum(not row["python_crashes_when_run"] for row in rows),
        "mypy_measured": have_mypy,
        "mypy_strict_catches": sum(bool(row["mypy_strict_catches"]) for row in rows) if have_mypy else None,
    }


def programs_with_output():
    """The conformance tests, with the text each must print."""
    return [(path, path.with_suffix(".out").read_text(encoding="utf-8")) for path in sorted((ROOT / "tests").glob("*.kilat"))]


def check_conformance():
    results = []
    for path, expected in programs_with_output():
        code, printed, _ = run_quiet([str(kilat_program()), "run", str(path)])
        results.append((f"tests/{path.name}", code == 0 and printed == expected))
    return results


def check_library_agrees():
    """The library, loaded by the Python binding, must print what the program prints."""
    binding = ROOT / "bindings" / "python" / "kilat.py"
    results = []
    sources = sorted((ROOT / "tests").glob("*.kilat")) + sorted((ROOT / "examples").glob("*.kilat"))
    for path in sources:
        _, native, _ = run_quiet([str(kilat_program()), "run", str(path)])
        _, through_library, _ = run_quiet([sys.executable, str(binding), str(path)])
        results.append((f"{path.parent.name}/{path.name}", native == through_library and native != ""))
    return results


def check_sandbox():
    folder = WORK / "sandbox"
    folder.mkdir(parents=True, exist_ok=True)
    kilat = str(kilat_program())
    programs = {
        "endless.kilat": "var turns = 0\nwhile true {\n    turns += 1\n}\n",
        "grow.kilat": "var xs: List[Int] = []\nwhile true {\n    push(xs, 1)\n}\n",
        "read.kilat": (
            'match read_file("secret.txt") {\n'
            '    Ok(text) => print("read:", text),\n'
            '    Err(message) => print("refused:", message),\n'
            "}\n"
        ),
        "write.kilat": (
            'match write_file("made.txt", "made by a program") {\n'
            '    Ok(_) => print("written"),\n'
            '    Err(message) => print("refused:", message),\n'
            "}\n"
        ),
    }
    for name, source in programs.items():
        (folder / name).write_text(source, encoding="utf-8", newline="\n")
    (folder / "secret.txt").write_text("the secret", encoding="utf-8")
    made = folder / "made.txt"
    made.unlink(missing_ok=True)
    results = []
    code, _, errors = run_quiet([kilat, "run", "endless.kilat", "--fuel", "1000000"], folder)
    results.append(("an endless loop is stopped at the fuel limit", code == 1 and "fuel limit" in errors))
    code, _, errors = run_quiet([kilat, "run", "grow.kilat", "--mem", "8"], folder)
    results.append(("a growing list is stopped at the memory limit", code == 1 and "out of memory" in errors))
    _, printed, _ = run_quiet([kilat, "run", "read.kilat"], folder)
    results.append(("a file cannot be read without --allow-read", printed.startswith("refused:") and "the secret" not in printed))
    _, printed, _ = run_quiet([kilat, "run", "read.kilat", "--allow-read"], folder)
    results.append(("--allow-read lets a file be read", printed == "read: the secret\n"))
    _, printed, _ = run_quiet([kilat, "run", "write.kilat"], folder)
    results.append(("a file cannot be written without --allow-write", printed.startswith("refused:") and not made.exists()))
    _, printed, _ = run_quiet([kilat, "run", "write.kilat", "--allow-write"], folder)
    results.append(("--allow-write lets a file be written", printed == "written\n" and made.is_file()))
    return results


def check_claims():
    """Small behaviours the README mentions, next to what Python does with the same input."""
    kilat = str(kilat_program())
    claims = [
        ("round_to(2.675, 2)", "print(round_to(2.675, 2))", "2.68", repr(round(2.675, 2))),
        ('"{1.005:.2}"', 'print("{1.005:.2}")', "1.01", f"{1.005:.2f}"),
        ("0.0 / 0.0", "let zero = 0.0\nprint(zero / zero)", "runtime error: division by zero", "ZeroDivisionError"),
        ("sqrt(-1.0)", "print(sqrt(-1.0))", "runtime error: sqrt needs a number that is not negative", "ValueError"),
        ("9223372036854775807 + 1", "print(9223372036854775807 + 1)", "runtime error: integer overflow", str(9223372036854775807 + 1)),
        ("a misspelt name", "let length = 3\nprint(lenght)", "did you mean `length`?", "NameError when the line runs"),
    ]
    results = []
    for label, source, expected, python_does in claims:
        _, printed, errors = run_quiet([kilat, "-e", source])
        results.append({"what": label, "kilat": expected, "python": python_does, "holds": expected in printed + errors})
    observations = {}
    for label, source in (
        ("1e308 * 10.0", "let big = 1e308\nprint(big * 10.0)"),
        ("infinity - infinity", "let big = 1e308\nlet huge = big * 10.0\nprint(huge - huge)"),
    ):
        _, printed, errors = run_quiet([kilat, "-e", source])
        observations[label] = (printed + errors).strip().splitlines()[0] if (printed + errors).strip() else ""
    return results, observations


def check_browser():
    """The engine, the playground and the Cairn plugin in a headless browser."""
    sys.path.insert(0, str(ROOT / "tests"))
    import page_test
    import web_test

    try:
        _report, web = web_test.run_checks()
        pages, timings = page_test.run_checks(timeout=240)
    except web_test.BrowserMissing as missing:
        return {"skipped": str(missing)}
    wasm_agrees = [(name, not problems) for name, problems in web if name.startswith(("tests/", "examples/"))]
    return {
        "web": [(name, not problems) for name, problems in web],
        "pages": [(f"{page}: {name}", not problems) for page, name, problems in pages],
        "wasm_agrees": wasm_agrees,
        "failures": [f"{name}: {problems}" for name, problems in web if problems]
        + [f"{page}: {name}: {problems}" for page, name, problems in pages if problems],
        "timings": timings,
    }


def count(results):
    return f"{sum(1 for _, passed in results if passed)} of {len(results)}"


def write_results(report):
    speed = report["speed"]
    sizes = report["sizes"]
    bugs = report["bugs"]
    lines = [
        "# Measured results",
        "",
        f"Written by `python judge.py` on {report['date']}. Kilat {report['kilat']}, "
        f"{report['python']}, {report['system']}, {report['processor']}.",
        "",
        "## Speed against CPython",
        "",
        f"Best of {speed['runs']} runs each. CPU time is the whole process, measured with `{speed['cpu_time_from']}`"
        + (", with both languages kept on the fastest cores." if speed["pinned_to_fastest_cores"] else "."),
        "\"Net\" takes each language's start-up time off first, which is the stricter figure for Kilat.",
        "Both languages run the same plain algorithm; Python with NumPy would be faster on some of these.",
        "Wall-clock time on the development PC is dominated by how long a process takes to start, so it is shown last.",
        "",
        "| Program | Kilat CPU s | CPython CPU s | Times faster | Net of start-up | Kilat wall s | CPython wall s |",
        "|---|---|---|---|---|---|---|",
    ]
    for row in speed["benchmarks"]:
        net = f"{row['ratio_net']:.2f}" if row["ratio_net"] else "n/a"
        lines.append(
            f"| `{row['name']}` | {row['kilat_cpu']:.3f} | {row['python_cpu']:.3f} | {row['ratio']:.2f} | {net} "
            f"| {row['kilat_wall']:.3f} | {row['python_wall']:.3f} |"
        )
    met = "met" if (speed["geometric_mean_net"] or 0) >= SPEED_TARGET else "not met"
    start = speed["startup"]
    lines += [
        "",
        f"- Geometric mean: {speed['geometric_mean']:.2f} times faster for the whole process, "
        f"{speed['geometric_mean_net']:.2f} net of start-up.",
        f"- Net of start-up Kilat is faster on {len(speed['faster'])} of {len(speed['benchmarks'])} programs"
        + (f", slower on {', '.join('`' + name + '`' for name in speed['slower'])}" if speed["slower"] else "")
        + ".",
        f"- The target set before measuring was {SPEED_TARGET:.0f} times: {met}.",
        f"- The two languages print the same text on every program: {'yes' if speed['outputs_match'] else 'NO'}.",
        f"- Start-up: Kilat {start['kilat_cpu'] * 1000:.0f} ms of CPU ({start['kilat_wall'] * 1000:.0f} ms wall), "
        f"CPython {start['python_cpu'] * 1000:.0f} ms ({start['python_wall'] * 1000:.0f} ms wall).",
        "",
        "## Sizes",
        "",
        "| File | Bytes |",
        "|---|---|",
    ]
    lines += [f"| {name} | {size:,} |" for name, size in sizes["bytes"].items()]
    for name, libraries in sizes["needs"].items():
        lines.append("")
        lines.append(f"The {name} needs only these libraries: {', '.join(libraries)}.")
    if sizes["needs"]:
        lines.append(f"All of them are part of Windows: {'yes' if sizes['needs_only_windows'] else 'NO'}.")
    lines.append(f"`docs/kilat.wasm` imports {sizes['wasm_imports']} things from its host.")
    lines += [
        "",
        "## Mistakes caught before the program runs",
        "",
        f"{bugs['total']} small programs in `bugs/`, each with one mistake, a fixed version and a Python twin.",
        "The corpus was written by the author of Kilat, so read it as a list of what Kilat checks, not as a survey.",
        "",
        "| Kind of mistake | Programs | Kilat refuses | mypy --strict reports | Python refuses before running |",
        "|---|---|---|---|---|",
    ]
    for category, group in bugs["categories"].items():
        mypy = group["mypy"] if bugs["mypy_measured"] else "not measured"
        lines.append(f"| {category} | {group['bugs']} | {group['kilat']} | {mypy} | {group['python_before_running']} |")
    mypy_total = bugs["mypy_strict_catches"] if bugs["mypy_measured"] else "not measured"
    lines += [
        f"| **all** | {bugs['total']} | {bugs['kilat_refuses']} | {mypy_total} | {bugs['python_refuses_before_running']} |",
        "",
        f"- Fixed versions that run: {bugs['fixed_run']} of {bugs['total']}.",
        f"- Python twins when run: {bugs['python_crashes_when_run']} stop with an error, "
        f"{bugs['python_runs_to_a_wrong_answer']} finish with a wrong or meaningless answer.",
        "- Python can check units while a program runs with a library such as pint; that is not measured here.",
        "",
        "## Checks",
        "",
        "| Check | Passed |",
        "|---|---|",
        f"| Conformance programs print their recorded output | {count(report['conformance'])} |",
        f"| The library prints what the program prints | {count(report['library_agrees'])} |",
        f"| Limits and file access on the command line | {count(report['sandbox'])} |",
        f"| Behaviours quoted in the README | {sum(claim['holds'] for claim in report['claims'])} of {len(report['claims'])} |",
    ]
    browser = report["browser"]
    if browser is None:
        lines.append("| Browser checks | not run (`--quick`) |")
    elif "skipped" in browser:
        lines.append(f"| Browser checks | skipped: {browser['skipped']} |")
    else:
        lines += [
            f"| WebAssembly build prints what the program prints | {count(browser['wasm_agrees'])} |",
            f"| Engine in a browser | {count(browser['web'])} |",
            f"| Playground and Cairn plugin pages | {count(browser['pages'])} |",
        ]
    lines += ["", "| Behaviour | Kilat | Python |", "|---|---|---|"]
    lines += [f"| `{claim['what']}` | {claim['kilat']} | {claim['python']} |" for claim in report["claims"]]
    lines += [f"| `{what}` | {seen} | |" for what, seen in report["observations"].items()]
    if browser and "timings" in browser:
        lines += ["", "Times seen in the browser checks, in milliseconds unless the name says otherwise:", ""]
        lines += [f"- `{name}`: {value}" for name, value in sorted(browser["timings"].items())]
    lines.append("")
    (ROOT / "RESULTS.md").write_text("\n".join(lines), encoding="utf-8", newline="\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--quick", action="store_true", help="fewer timing runs and no browser sections")
    parser.add_argument("--runs", type=int, default=0, help="timing runs per program (default 5, or 2 with --quick)")
    options = parser.parse_args()
    if not kilat_program().is_file():
        print("build Kilat first: python build.py")
        return 2
    runs = options.runs or (2 if options.quick else 5)
    name, cycles_per_second = processor()
    claims, observations = check_claims()
    report = {
        "date": time.strftime("%Y-%m-%d"),
        "kilat": run_quiet([str(kilat_program()), "version"])[1].strip(),
        "python": f"{platform.python_implementation()} {platform.python_version()}",
        "system": f"{platform.system()} {platform.release()}",
        "processor": name,
        "speed": measure_speed(runs, cycles_per_second),
        "sizes": measure_sizes(),
        "bugs": measure_bugs(),
        "conformance": check_conformance(),
        "library_agrees": check_library_agrees(),
        "sandbox": check_sandbox(),
        "claims": claims,
        "observations": observations,
        "browser": None if options.quick else check_browser(),
    }
    failures = []
    for section in ("conformance", "library_agrees", "sandbox"):
        failures += [f"{section}: {label}" for label, passed in report[section] if not passed]
    failures += [f"claim: {claim['what']}" for claim in claims if not claim["holds"]]
    bugs = report["bugs"]
    if bugs["kilat_refuses"] != bugs["total"] or bugs["fixed_run"] != bugs["total"]:
        failures.append("bugs: Kilat did not refuse every bug, or a fixed version did not run")
    if not report["speed"]["outputs_match"]:
        failures.append("speed: a benchmark printed different text in the two languages")
    if report["sizes"]["needs"] and not report["sizes"]["needs_only_windows"]:
        failures.append("sizes: the program needs a library that is not part of Windows")
    if report["sizes"]["wasm_imports"]:
        failures.append("sizes: the WebAssembly build imports something")
    browser = report["browser"]
    if browser and "failures" in browser:
        failures += browser["failures"]
    report["failures"] = failures
    (ROOT / "judge_report.json").write_text(json.dumps(report, indent=1), encoding="utf-8", newline="\n")
    write_results(report)
    speed = report["speed"]
    print(f"speed: {speed['geometric_mean']:.2f} times CPython (whole process), {speed['geometric_mean_net']:.2f} net of start-up")
    print(f"bugs refused before running: Kilat {bugs['kilat_refuses']}/{bugs['total']}, mypy --strict {bugs['mypy_strict_catches']}")
    for failure in failures:
        print(f"FAIL  {failure}")
    print("every check passed" if not failures else f"{len(failures)} checks failed")
    print("wrote judge_report.json and RESULTS.md")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
