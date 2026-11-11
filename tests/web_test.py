"""Runs Kilat programs through docs/kilat.js in a headless browser and checks the results.

    python tests/web_test.py

Standard library only. It needs Microsoft Edge, Google Chrome or Chromium (set KILAT_BROWSER
to the program to use another copy); without one it says so and exits with code 2. The
browser gets a throw-away profile in judge_work/ and talks only to a server on 127.0.0.1
that this script starts.
"""

import functools
import http.server
import itertools
import json
import os
import queue
import shutil
import subprocess
import sys
import threading
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORK = ROOT / "judge_work"

BROWSER_COMMANDS = (
    "msedge",
    "microsoft-edge",
    "google-chrome",
    "chrome",
    "chromium",
    "chromium-browser",
)
WINDOWS_BROWSERS = (
    r"Microsoft\Edge\Application\msedge.exe",
    r"Google\Chrome\Application\chrome.exe",
)
MAC_BROWSERS = (
    "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
)


LAUNCH_NUMBERS = itertools.count(1)


class BrowserMissing(Exception):
    """No browser that can run headless was found."""


def find_browser():
    """The path of a Chromium-family browser, or None."""
    override = os.environ.get("KILAT_BROWSER")
    if override:
        return override
    for command in BROWSER_COMMANDS:
        found = shutil.which(command)
        if found:
            return found
    candidates = [Path(path) for path in MAC_BROWSERS]
    for variable in ("ProgramFiles(x86)", "ProgramFiles", "LocalAppData"):
        base = os.environ.get(variable)
        if base:
            candidates.extend(Path(base) / tail for tail in WINDOWS_BROWSERS)
    for candidate in candidates:
        if candidate.is_file():
            return str(candidate)
    return None


class Handler(http.server.SimpleHTTPRequestHandler):
    """Serves the repository, hands out the cases and collects the report."""

    # Windows may map .js to text/plain in the registry, and a browser refuses to load a
    # module with that type.
    extensions_map = {
        ".html": "text/html; charset=utf-8",
        ".js": "text/javascript; charset=utf-8",
        ".json": "application/json",
        ".wasm": "application/wasm",
    }

    def reply(self, body, content_type):
        self.send_response(200)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        if self.path == "/cases.json":
            self.reply(self.server.cases_json, "application/json")
        else:
            super().do_GET()

    def do_POST(self):
        length = int(self.headers.get("Content-Length", "0"))
        body = self.rfile.read(length)
        self.reply(b"{}", "application/json")
        if self.path == "/report":
            self.server.reports.put(json.loads(body))
        elif self.path == "/progress":
            self.server.progress.append(body.decode("utf-8", "replace"))

    def log_message(self, *arguments):
        pass


def start_browser(command):
    """Starts the browser so that stop_browser can end it together with its helpers.

    Returns (process, group). A browser starts helper processes (renderer, crash handler)
    that outlive it when only the main process is ended, so on Windows they all go into
    one job object, and elsewhere into one process group.
    """
    quiet = {"stdout": subprocess.DEVNULL, "stderr": subprocess.DEVNULL}
    if sys.platform != "win32":
        return subprocess.Popen(command, start_new_session=True, **quiet), None
    import ctypes
    from ctypes import wintypes

    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel32.CreateJobObjectW.restype = wintypes.HANDLE
    kernel32.CreateJobObjectW.argtypes = (ctypes.c_void_p, wintypes.LPCWSTR)
    kernel32.AssignProcessToJobObject.argtypes = (wintypes.HANDLE, wintypes.HANDLE)
    job = kernel32.CreateJobObjectW(None, None)
    process = subprocess.Popen(command, **quiet)
    # The helpers are started later, by the browser itself, and inherit the job.
    if job and not kernel32.AssignProcessToJobObject(job, wintypes.HANDLE(int(process._handle))):
        kernel32.CloseHandle(wintypes.HANDLE(job))
        job = None
    return process, job


def stop_browser(process, group):
    """Ends the browser and its helpers. Never raises and never waits long.

    Ending a process can take many seconds on a loaded PC. The report is already in by
    then, so a browser that is still on its way out is left to finish by itself.
    """
    try:
        if sys.platform != "win32":
            import signal

            os.killpg(process.pid, signal.SIGKILL)
        elif group:
            import ctypes
            from ctypes import wintypes

            kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
            kernel32.TerminateJobObject.argtypes = (wintypes.HANDLE, wintypes.UINT)
            kernel32.CloseHandle.argtypes = (wintypes.HANDLE,)
            kernel32.TerminateJobObject(group, 1)
            kernel32.CloseHandle(group)
        else:
            process.terminate()
    except OSError:
        pass
    try:
        process.wait(timeout=3)
    except subprocess.TimeoutExpired:
        pass


def remove_folder(folder):
    """Removes `folder` if it can; a browser lets go of its files a moment after it stops."""
    for _ in range(12):
        shutil.rmtree(folder, ignore_errors=True)
        if not folder.exists():
            return True
        time.sleep(0.25)
    return False


def remove_stale_profiles(older_than_seconds=600.0):
    """Clears browser profiles that an earlier run could not remove in time."""
    if not WORK.is_dir():
        return
    for folder in WORK.glob("browser-profile-*"):
        try:
            age = time.time() - folder.stat().st_mtime
        except OSError:
            continue
        if age > older_than_seconds:
            shutil.rmtree(folder, ignore_errors=True)


def run_in_browser(cases, timeout=120.0, browser=None, page="tests/web_runner.html", root=ROOT):
    """Runs `cases` on `page` and returns what the page reports.

    A case is {"name", "source"} plus optional "mode" ("check"), "options" (passed to
    kilat.run), "host" ("echo", "async-echo" or "throw") and "abortAfterMs". The report is
    {"version", "userAgent", "results"}; a result is what kilat.run returned plus "name",
    "chunkCount", "chunksMatchOutput" and "milliseconds".
    """
    browser = browser or find_browser()
    if browser is None:
        raise BrowserMissing("no Edge, Chrome or Chromium was found (set KILAT_BROWSER)")
    handler = functools.partial(Handler, directory=str(root))
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
    server.cases_json = json.dumps(cases).encode("utf-8")
    server.reports = queue.Queue()
    server.progress = []
    threading.Thread(target=server.serve_forever, daemon=True).start()
    remove_stale_profiles()
    # A new folder for every launch: a browser that is still shutting down keeps its folder
    # locked, and a second browser started on that folder exits without opening the page.
    profile = WORK / f"browser-profile-{os.getpid()}-{next(LAUNCH_NUMBERS)}"
    profile.mkdir(parents=True, exist_ok=True)
    command = [
        browser,
        "--headless=new",
        "--disable-gpu",
        "--no-first-run",
        "--no-default-browser-check",
        "--disable-extensions",
        "--no-proxy-server",
        f"--user-data-dir={profile}",
        f"http://127.0.0.1:{server.server_address[1]}/{page}",
    ]
    process, group = start_browser(command)
    try:
        report = server.reports.get(timeout=timeout)
    except queue.Empty:
        last = server.progress[-1] if server.progress else "none"
        raise TimeoutError(
            f"the browser did not report within {timeout:.0f} s (last finished check: {last})"
        ) from None
    finally:
        stop_browser(process, group)
        server.shutdown()
        server.server_close()
        remove_folder(profile)
    if "fatal" in report:
        raise RuntimeError(f"the page failed: {report['fatal']}")
    return report


def native_kilat():
    """The native kilat program built by cargo, or None."""
    name = "kilat.exe" if sys.platform == "win32" else "kilat"
    path = ROOT / "target" / "release" / name
    return path if path.is_file() else None


def program_cases():
    """The conformance tests and the examples, each with the output it must print."""
    pairs = []
    for path in sorted((ROOT / "tests").glob("*.kilat")):
        expected = path.with_suffix(".out").read_text(encoding="utf-8")
        pairs.append((path, expected))
    native = native_kilat()
    for path in sorted((ROOT / "examples").glob("*.kilat")):
        expected = None
        if native:
            finished = subprocess.run([str(native), "run", str(path)], capture_output=True, timeout=60)
            expected = finished.stdout.decode("utf-8").replace("\r\n", "\n")
        pairs.append((path, expected))
    cases = []
    for path, expected in pairs:
        case = {
            "name": f"{path.parent.name}/{path.name}",
            "source": path.read_text(encoding="utf-8"),
        }
        expectation = {"ok": True, "kind": "", "error": "", "chunksMatchOutput": True}
        if expected is not None:
            expectation["output"] = expected
        cases.append((case, expectation))
    return cases


ENDLESS = "var turns = 0\nwhile true {\n    turns += 1\n}\n"
# Every turn copies a long text. A fixed number of turns per step would freeze the page.
SLOW_TURNS = (
    'let text = repeat("ab", 100000)\nvar total = 0\nwhile true {\n'
    '    total += len(replace(text, "a", "c"))\n}\n'
)
# Every turn asks the page for the time. Answering must not keep the page from its other work.
ASKING_TURNS = "var total = 0.0\nwhile true {\n    total += clock()\n}\n"
SHOW_HOST = (
    'match host("greet", "abc") {\n'
    '    Ok(text) => print("ok", text),\n'
    '    Err(message) => print("err", message),\n'
    "}\n"
)
ROLL = "print(rand_int(1, 1000000000), rand_int(1, 1000000000))\n"
DEEP = "print(" + "(" * 20000 + "1" + ")" * 20000 + ")\n"


def api_cases():
    """Cases for docs/kilat.js itself: limits, the host gate, options and stopping."""
    return [
        (
            {"name": "check finds a unit error", "mode": "check", "source": "let gap: mm = 2 s\n"},
            {"ok": False, "kind": "compile", "output": "", "error_contains": "unit mismatch"},
        ),
        (
            {"name": "check does not run the program", "mode": "check", "source": 'print("ran")\n'},
            {"ok": True, "kind": "", "output": ""},
        ),
        (
            {"name": "run reports a compile error", "source": "print(missing)\n"},
            {"ok": False, "kind": "compile", "output": ""},
        ),
        (
            {"name": "runtime error keeps earlier output", "source": 'print("before")\nlet xs = [1, 2, 3]\nprint(xs[10])\n'},
            {"ok": False, "kind": "runtime", "output": "before\n", "error_contains": "index 10 is out of range"},
        ),
        (
            {"name": "fuel limit stops an endless loop", "source": ENDLESS, "options": {"fuel": 100000}},
            {"ok": False, "kind": "fuel", "error_contains": "fuel limit"},
        ),
        (
            {
                "name": "memory limit stops a growing list",
                "source": "var xs: List[Int] = []\nwhile true {\n    push(xs, 1)\n}\n",
                "options": {"memoryMb": 8, "fuel": 1e12},
            },
            {"ok": False, "kind": "memory", "error_contains": "out of memory"},
        ),
        (
            {"name": "stop signal ends an endless loop", "source": ENDLESS, "options": {"fuel": 1e15}, "abortAfterMs": 300},
            {"ok": False, "kind": "stopped"},
        ),
        (
            {"name": "stop signal ends a loop of slow turns", "source": SLOW_TURNS, "options": {"fuel": 1e15}, "abortAfterMs": 300},
            {"ok": False, "kind": "stopped", "max_ms": 3000},
        ),
        (
            {"name": "stop signal ends a loop of host calls", "source": ASKING_TURNS, "options": {"fuel": 1e15}, "abortAfterMs": 300},
            {"ok": False, "kind": "stopped", "max_ms": 3000},
        ),
        (
            {"name": "host request is refused by default", "source": SHOW_HOST},
            {"ok": True, "output": "err `greet` is not allowed here\n"},
        ),
        (
            {"name": "host function answers", "source": SHOW_HOST, "host": "echo"},
            {"ok": True, "output": "ok echo:greet:abc\n"},
        ),
        (
            {"name": "async host function answers", "source": SHOW_HOST, "host": "async-echo"},
            {"ok": True, "output": "ok echo:greet:abc\n"},
        ),
        (
            {"name": "host function that throws gives Err", "source": SHOW_HOST, "host": "throw"},
            {"ok": True, "output": "err nope\n"},
        ),
        (
            {"name": "input is refused without a host", "source": 'print("[" + input("name? ") + "]")\n'},
            {"ok": True, "output": "[]\n"},
        ),
        (
            {"name": "clock works without a host", "source": "print(clock() >= 0.0)\n"},
            {"ok": True, "output": "true\n"},
        ),
        (
            {
                "name": "files option feeds import",
                "source": 'import "lib.kilat"\nprint(twice(21))\n',
                "options": {"files": {"lib.kilat": "fn twice(x: Int) -> Int {\n    x * 2\n}\n"}},
            },
            {"ok": True, "output": "42\n"},
        ),
        (
            {"name": "import without the file is an error", "source": 'import "lib.kilat"\nprint(twice(21))\n'},
            {"ok": False, "kind": "compile"},
        ),
        (
            {"name": "printLast prints the final value", "source": "1 + 2\n", "options": {"printLast": True}},
            {"ok": True, "output": "3\n"},
        ),
        (
            {"name": "without printLast nothing is printed", "source": "1 + 2\n"},
            {"ok": True, "output": ""},
        ),
        (
            {"name": "text survives the trip", "source": 'print("h\u00e9llo \u2713 \u65e5\u672c", len("\u65e5\u672c"))\n'},
            {"ok": True, "output": "h\u00e9llo \u2713 \u65e5\u672c 2\n"},
        ),
        (
            {
                "name": "output arrives in pieces",
                "source": "for i in 0..300000 {\n    if i % 1000 == 0 {\n        print(i)\n    }\n}\n",
            },
            {"ok": True, "chunksMatchOutput": True},
        ),
        ({"name": "seed 7", "source": ROLL, "options": {"seed": 7}}, {"ok": True}),
        ({"name": "seed 7 again", "source": ROLL, "options": {"seed": 7}}, {"ok": True}),
        ({"name": "seed 8", "source": ROLL, "options": {"seed": 8}}, {"ok": True}),
        (
            {"name": "very deep nesting is contained", "source": DEEP},
            {"ok": False, "kind_in": ("compile", "crash")},
        ),
        (
            {"name": "a run after that still works", "source": 'print("still fine")\n'},
            {"ok": True, "output": "still fine\n"},
        ),
    ]


def short(value, limit=110):
    text = repr(value)
    return text if len(text) <= limit else text[: limit - 3] + "..."


def problems_with(result, expectation):
    """Why `result` does not meet `expectation`; an empty list when it does."""
    if "threw" in result:
        return [f"the page threw: {result['threw']}"]
    problems = []
    for key, wanted in expectation.items():
        if key == "error_contains":
            if wanted not in result["error"]:
                problems.append(f"error {short(result['error'])} does not mention {wanted!r}")
        elif key == "kind_in":
            if result["kind"] not in wanted:
                problems.append(f"kind is {result['kind']!r}, expected one of {wanted}")
        elif key == "max_ms":
            if result["milliseconds"] > wanted:
                problems.append(f"it took {result['milliseconds']:.0f} ms, expected at most {wanted}")
        elif result[key] != wanted:
            problems.append(f"{key} is {short(result[key])}, expected {short(wanted)}")
    return problems


def run_checks(timeout=120.0):
    """Runs every case; returns (report, [(name, problems)]) in case order."""
    pairs = program_cases() + api_cases()
    report = run_in_browser([case for case, _ in pairs], timeout=timeout)
    by_name = {result["name"]: result for result in report["results"]}
    outcomes = []
    for case, expectation in pairs:
        result = by_name.get(case["name"])
        problems = ["no result came back"] if result is None else problems_with(result, expectation)
        outcomes.append((case["name"], problems))
    seeds = [by_name.get(name, {}).get("output") for name in ("seed 7", "seed 7 again", "seed 8")]
    seed_problems = []
    if seeds[0] != seeds[1]:
        seed_problems.append(f"the same seed gave {short(seeds[0])} and then {short(seeds[1])}")
    if seeds[0] == seeds[2]:
        seed_problems.append(f"two different seeds both gave {short(seeds[0])}")
    outcomes.append(("the same seed repeats and another seed differs", seed_problems))
    return report, outcomes


def main():
    try:
        report, outcomes = run_checks()
    except BrowserMissing as missing:
        print(f"skipped: {missing}")
        return 2
    failed = 0
    for name, problems in outcomes:
        if problems:
            failed += 1
            print(f"FAIL  {name}")
            for problem in problems:
                print(f"        {problem}")
        else:
            print(f"ok    {name}")
    print(f"{len(outcomes) - failed} of {len(outcomes)} web checks passed (Kilat {report['version']})")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
