"""How often does a free model write a working Kilat program on the live GlowCode page?

    python tests/glowcode_model_sample.py <report.json>

GlowCode (https://juw12.github.io/projects/glow/code/) can ask a free model to write the Kilat
program for a request typed in plain words. The page shows each program to Kilat's checker and hands
a refusal back to the model, up to three tries. This script types engineering requests with a known
answer into the real page in a real browser and records what happens. Nothing is mocked: the page,
the checker and the model answers are whatever the live sites send.

Each run ends in exactly one of these outcomes:
    correct        the checker accepted a program, it ran, and the printed numbers match the answer
    ran_wrong      the checker accepted a program, but it failed or printed other numbers
    refused        a model answered, but the checker accepted none of its tries
    no_model       no model answered at all (the free providers failed or refused the caller)
    timeout        the page did not finish in time and Stop was pressed
    harness_error  this script broke on that run (it is tried twice)

For every program the checker refused, the report keeps the program and the checker's message. The
message is read by showing the same program to the page's own engine once more after the run. The
requests the page sends to the model providers are not read or stored.

This is a measurement of a small sample, not a pass / fail test: it exits 1 only when the rig itself
did not work (no browser, or no run reached the page). When the first three runs all end in
harness_error it stops there, so a broken rig does not use up the whole time budget. Know its limits
before quoting it:
  - free models change from day to day, and a provider may refuse the address the browser runs on;
  - a run counts as correct when every expected number is printed somewhere in the output, within
    1.5 %, so a wrong program can match by accident;
  - the page's instructions to the model contain worked numbers; `numbers_also_in_prompt` lists, per
    task, the numbers of the request and of the answer that also stand in those instructions.

It needs Playwright:  pip install playwright   then   python -m playwright install chromium

Environment:
    KILAT_PAGE_URL               the page to open (default: the live page)
    KILAT_SAMPLE_SET             main (default, ten tasks) or heldout (fourteen other tasks)
    KILAT_SAMPLE_TASKS           comma-separated task names, to ask only those
    KILAT_SAMPLE_REPEATS         how often each task is asked (default 3)
    KILAT_SAMPLE_PAUSE           seconds to wait between runs (default 45)
    KILAT_SAMPLE_BUDGET_MINUTES  start no new run after this many minutes (default 200)
"""

import hashlib
import json
import os
import platform
import re
import sys
import time
import traceback
from pathlib import Path
from urllib.parse import urlsplit

from playwright.sync_api import sync_playwright

PAGE_URL = os.environ.get("KILAT_PAGE_URL") or "https://juw12.github.io/projects/glow/code/"
PAGE_HOST = urlsplit(PAGE_URL).hostname or ""
SAMPLE_SET = os.environ.get("KILAT_SAMPLE_SET") or "main"
REPEATS = int(os.environ.get("KILAT_SAMPLE_REPEATS") or "3")
PAUSE_SECONDS = float(os.environ.get("KILAT_SAMPLE_PAUSE") or "45")
BUDGET_MINUTES = float(os.environ.get("KILAT_SAMPLE_BUDGET_MINUTES") or "200")
LOAD_SECONDS = 120
MODEL_WAIT_SECONDS = 240
RELATIVE_TOLERANCE = 0.015
OUTCOMES = ["correct", "ran_wrong", "refused", "no_model", "timeout", "harness_error"]

# (short name, the request typed into the page, the numbers the output must contain)
MAIN_TASKS = [
    (
        "rod stress",
        "A 1.5 kN load hangs on a round steel rod 6 mm in diameter. Print the stress in MPa.",
        [53.05],
    ),
    (
        "ohm",
        "A 12 V supply feeds a 470 ohm resistor. Print the current in mA and the power in mW.",
        [25.53, 306.4],
    ),
    (
        "box volume",
        "A box is 300 mm by 200 mm by 150 mm. Print its volume in litres.",
        [9.0],
    ),
    (
        "shaft torque",
        "A shaft turns at 1450 rpm and carries 7.5 kW. Print the torque in N*m with one decimal.",
        [49.39],
    ),
    (
        "average speed",
        "A car travels 150 km in 2.5 hours. Print the average speed in m/s with two decimals.",
        [16.67],
    ),
    (
        "spring",
        "A spring with stiffness 12 N/mm is compressed by 8 mm. "
        "Print the force in N and the stored energy in J.",
        [96.0, 0.384],
    ),
    (
        "beam inertia",
        "A rectangular beam section is 40 mm wide and 60 mm high. "
        "Print its second moment of area about the horizontal axis in mm^4.",
        [720000.0],
    ),
    (
        "readings",
        "Five gap readings are 2.1, 2.4, 1.9, 2.2 and 2.0 mm. "
        "Print the mean with two decimals and the largest reading.",
        [2.12, 2.4],
    ),
    (
        "sum to 100",
        "Print the sum of the whole numbers from 1 to 100.",
        [5050.0],
    ),
    (
        "tank fill",
        "A pump delivers 40 litres per minute into an empty 2 cubic metre tank. "
        "Print the time to fill it in minutes.",
        [50.0],
    ),
]

# Held-out tasks: the same kinds of question with other numbers, and four kinds the main set does
# not have (shaft power, piston force, plate pressure, kinetic energy). The numbers were chosen to
# differ from the worked numbers in the page's instructions; `numbers_also_in_prompt` in the report
# shows what still overlaps.
HELDOUT_TASKS = [
    (
        "motor torque",
        "A motor turns at 960 rpm and delivers 3 kW. Print the torque in N*m with two decimals.",
        [29.84],
    ),
    (
        "spring 2",
        "A spring with stiffness 4.5 N/mm is compressed by 20 mm. "
        "Print the force in N and the stored energy in J.",
        [90.0, 0.9],
    ),
    (
        "shaft power",
        "A shaft carries 35 N*m of torque at 2900 rpm. Print the power in kW with two decimals.",
        [10.63],
    ),
    (
        "piston force",
        "A pressure of 2.5 MPa acts on a round piston 80 mm in diameter. "
        "Print the force in kN with two decimals.",
        [12.57],
    ),
    (
        "rod stress 2",
        "A 2.4 kN load hangs on a round steel rod 8 mm in diameter. Print the stress in MPa.",
        [47.75],
    ),
    (
        "ohm 2",
        "A 9 V supply feeds a 330 ohm resistor. Print the current in mA and the power in mW.",
        [27.27, 245.45],
    ),
    (
        "box volume 2",
        "A box is 450 mm by 280 mm by 120 mm. Print its volume in litres.",
        [15.12],
    ),
    (
        "average speed 2",
        "A cyclist travels 42 km in 1.75 hours. Print the average speed in m/s with two decimals.",
        [6.67],
    ),
    (
        "beam inertia 2",
        "A rectangular beam section is 30 mm wide and 80 mm high. "
        "Print its second moment of area about the horizontal axis in mm^4.",
        [1280000.0],
    ),
    (
        "readings 2",
        "Six gap readings are 3.2, 2.9, 3.4, 3.1, 3.0 and 3.3 mm. "
        "Print the mean with two decimals and the largest reading.",
        [3.15, 3.4],
    ),
    (
        "sum to 300",
        "Print the sum of the whole numbers from 1 to 300.",
        [45150.0],
    ),
    (
        "tank fill 2",
        "A pump delivers 32 litres per minute into an empty 2.4 cubic metre tank. "
        "Print the time to fill it in minutes.",
        [75.0],
    ),
    (
        "plate pressure",
        "A force of 1.8 kN presses evenly on a plate 60 mm by 40 mm. "
        "Print the pressure in MPa with two decimals.",
        [0.75],
    ),
    (
        "kinetic energy",
        "A 1200 kg car moves at 25 m/s. Print its kinetic energy in kJ.",
        [375.0],
    ),
]

READY = "() => ['ready', 'failed'].includes(document.body.dataset.run)"
DONE = "() => document.body.dataset.run === 'done'"
RUN_ENABLED = "() => !document.getElementById('run-btn').disabled"

# Keeps every program the checker refused. The page puts a program into the code box before it
# checks it and writes the log line right after the check, so when a "refused" line appears the
# code box still holds that program.
WATCH_REFUSALS = """() => {
  window.kilatRefused = [];
  const watcher = new MutationObserver((changes) => {
    for (const change of changes) {
      for (const node of change.addedNodes) {
        if (node.nodeType !== 1 || node.dataset.verdict !== 'fail') continue;
        if (!node.textContent.includes('wrote a program')) continue;
        window.kilatRefused.push({
          step: node.textContent,
          code: document.getElementById('code-box').value,
        });
      }
    }
  });
  watcher.observe(document.getElementById('model-log'), { childList: true });
}"""

# Shows the refused programs to the page's own engine again and returns the checker's messages.
CHECK_AGAIN = """async (programs) => {
  const { loadKilat } = await import(new URL('kilat.js', location.href).href);
  const kilat = await loadKilat();
  const messages = [];
  for (const program of programs) {
    const verdict = await kilat.check(program);
    messages.push(verdict.ok ? '(accepted this time)' : String(verdict.error || '').slice(0, 600));
  }
  return messages;
}"""

READ_RESULT = """() => {
  const shown = (id) => {
    const node = document.getElementById(id);
    return node && !node.hidden ? node.textContent : '';
  };
  return {
    run: document.body.dataset.run,
    kind: document.getElementById('result').dataset.kind || '',
    status: document.getElementById('result-status').textContent,
    output: shown('result-output'),
    error: shown('result-error'),
    steps: [...document.querySelectorAll('#model-log li')].map((li) => ({
      text: li.textContent,
      verdict: li.dataset.verdict || '',
    })),
    code: document.getElementById('code-box').value,
  };
}"""

STEP = re.compile(r"^Try (\d+): (.*?) (?:wrote a program|sent no program)")


def numbers_in(text):
    """Every number written in `text`; a comma between digit groups is read as a thousands mark."""
    cleaned = re.sub(r"(?<=\d),(?=\d{3}\b)", "", text)
    found = []
    for token in re.findall(r"-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?", cleaned):
        try:
            found.append(float(token))
        except ValueError:
            continue
    return found


def matches(expected, printed):
    """True when every expected number is printed, within the relative tolerance."""
    for wanted in expected:
        allowed = abs(wanted) * RELATIVE_TOLERANCE
        if not any(abs(value - wanted) <= allowed for value in printed):
            return False
    return True


def classify(shown, expected):
    if shown["timed_out"]:
        return "timeout"
    if not shown["steps"]:
        return "no_model"
    if not any(step["verdict"] == "pass" for step in shown["steps"]):
        return "refused"
    if shown["kind"] != "ok":
        return "ran_wrong"
    return "correct" if matches(expected, numbers_in(shown["output"])) else "ran_wrong"


def models_of(steps):
    """The model names in the page's log lines, in the order they first answered."""
    names = []
    for step in steps:
        found = STEP.match(step["text"])
        if found and found.group(2) not in names:
            names.append(found.group(2))
    return names


def chosen_tasks():
    if SAMPLE_SET not in ("main", "heldout"):
        raise SystemExit(f"KILAT_SAMPLE_SET must be main or heldout, not {SAMPLE_SET!r}")
    tasks = MAIN_TASKS if SAMPLE_SET == "main" else HELDOUT_TASKS
    wanted = os.environ.get("KILAT_SAMPLE_TASKS", "")
    only = [name.strip() for name in wanted.split(",") if name.strip()]
    if not only:
        return tasks
    unknown = [name for name in only if name not in [task[0] for task in tasks]]
    if unknown:
        raise SystemExit(f"unknown task names: {unknown}")
    return [task for task in tasks if task[0] in only]


def prompt_of(code_js):
    """The page's instructions to the model, cut out of its code.js; '' when they are not found."""
    start = code_js.find("const SYSTEM_PROMPT = [")
    end = code_js.find('].join("\\n");', start)
    return code_js[start:end] if start >= 0 and end > start else ""


def also_in_prompt(asked, expected, prompt_numbers):
    """The numbers of a request, and of its answer, that also stand in the page's instructions."""
    asked_shared = []
    for value in numbers_in(asked):
        if value in prompt_numbers and value not in asked_shared:
            asked_shared.append(value)
    answer_shared = [value for value in expected if matches([value], prompt_numbers)]
    return {"asked": asked_shared, "answer": answer_shared}


def one_run(context, repeat, name, asked, expected):
    page = context.new_page()
    provider_statuses = []
    failed_requests = []

    # Only the host and the status are kept: no address, no header and no body of any request.
    def on_response(response):
        host = urlsplit(response.url).hostname or ""
        if host != PAGE_HOST:
            provider_statuses.append({"host": host, "status": response.status})

    def on_failed(request):
        host = urlsplit(request.url).hostname or ""
        if host != PAGE_HOST:
            failed_requests.append({"host": host, "why": (request.failure or "")[:100]})

    page.on("response", on_response)
    page.on("requestfailed", on_failed)
    try:
        page.goto(PAGE_URL, wait_until="domcontentloaded", timeout=LOAD_SECONDS * 1000)
        page.wait_for_function(READY, timeout=LOAD_SECONDS * 1000)
        engine = page.text_content("#code-meta") or ""
        page.click('#brain-seg button[data-brain="free"]')
        page.fill("#task-box", asked)
        page.evaluate(WATCH_REFUSALS)
        page.wait_for_function(RUN_ENABLED, timeout=60000)
        began = time.time()
        page.click("#run-btn", timeout=60000)
        timed_out = False
        try:
            page.wait_for_function(DONE, timeout=MODEL_WAIT_SECONDS * 1000)
        except Exception:  # noqa: BLE001 - press Stop and record what the page shows
            timed_out = True
            page.click("#stop-btn", timeout=30000)
            page.wait_for_function(DONE, timeout=30000)
        shown = page.evaluate(READ_RESULT)
        shown["seconds"] = round(time.time() - began, 1)
        refusals = page.evaluate("() => window.kilatRefused || []")
        if refusals:
            try:
                messages = page.evaluate(CHECK_AGAIN, [refusal["code"] for refusal in refusals])
                for refusal, message in zip(refusals, messages):
                    refusal["error"] = message
            except Exception as problem:  # noqa: BLE001 - the run itself still counts
                shown["check_again_problem"] = repr(problem)[:300]
    finally:
        page.close()
    shown["timed_out"] = timed_out
    shown["engine"] = engine.strip()
    shown["task"] = name
    shown["repeat"] = repeat
    shown["expected"] = expected
    shown["printed_numbers"] = numbers_in(shown["output"])
    shown["outcome"] = classify(shown, expected)
    passing = [step for step in shown["steps"] if step["verdict"] == "pass"]
    shown["tries"] = len(shown["steps"])
    shown["first_try"] = bool(passing) and passing[0]["text"].startswith("Try 1:")
    shown["models"] = models_of(shown["steps"])
    shown["refusals"] = refusals
    shown["provider_statuses"] = provider_statuses
    shown["failed_requests"] = failed_requests
    return shown


def run_twice(context, repeat, name, asked, expected):
    """One run; a run this script broke on is tried once more."""
    first_problem = ""
    for attempt in (1, 2):
        try:
            shown = one_run(context, repeat, name, asked, expected)
            shown["attempts"] = attempt
            if first_problem:
                shown["first_attempt_problem"] = first_problem
            return shown
        except Exception as problem:  # noqa: BLE001 - record the run as broken and go on
            last = {
                "task": name,
                "repeat": repeat,
                "expected": expected,
                "outcome": "harness_error",
                "attempts": attempt,
                "detail": repr(problem)[:300],
                "trace": traceback.format_exc()[-800:],
            }
            first_problem = first_problem or last["detail"]
    return last


def start_browser(playwright, problems):
    # The Chromium that Playwright installs comes first; an installed Edge or Chrome also does.
    for channel in (None, "msedge", "chrome"):
        try:
            if channel:
                browser = playwright.chromium.launch(channel=channel, headless=True)
            else:
                browser = playwright.chromium.launch(headless=True)
            return browser, channel or "bundled chromium"
        except Exception as problem:  # noqa: BLE001 - try the next browser
            problems.append(f"browser {channel or 'bundled chromium'}: {repr(problem)[:200]}")
    raise RuntimeError("no browser could be started")


def main():
    if len(sys.argv) != 2:
        print(__doc__)
        return 2
    report_path = Path(sys.argv[1])
    tasks = chosen_tasks()
    started = time.time()
    report = {
        "page": PAGE_URL,
        "set": SAMPLE_SET,
        "repeats": REPEATS,
        "pause_seconds": PAUSE_SECONDS,
        "budget_minutes": BUDGET_MINUTES,
        "tolerance": RELATIVE_TOLERANCE,
        "model_wait_seconds": MODEL_WAIT_SECONDS,
        "runner": platform.platform(),
        "started": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "tasks": [{"name": name, "asked": asked, "expected": expected} for name, asked, expected in tasks],
        "runs": [],
        "problems": [],
    }

    def save():
        counts = {}
        for run in report["runs"]:
            counts[run["outcome"]] = counts.get(run["outcome"], 0) + 1
        report["counts"] = counts
        report["total"] = len(report["runs"])
        report["planned"] = len(tasks) * REPEATS
        report["seconds"] = round(time.time() - started, 1)
        report_path.parent.mkdir(parents=True, exist_ok=True)
        report_path.write_text(json.dumps(report, indent=1, ensure_ascii=False), encoding="utf-8")

    playwright = sync_playwright().start()
    browser = None
    try:
        browser, channel = start_browser(playwright, report["problems"])
        report["browser"] = {"channel": channel, "version": browser.version}
        report["problems"] = []  # a browser started, so the earlier tries do not matter
        context = browser.new_context(viewport={"width": 1280, "height": 900})

        # Which page code is being sampled, and which numbers its instructions already hold.
        try:
            code_js = context.request.get(PAGE_URL + "code.js", timeout=60000)
            body = code_js.body() if code_js.ok else b""
            prompt = prompt_of(body.decode("utf-8", "replace"))
            prompt_numbers = sorted(set(numbers_in(prompt)))
            report["page_code_js"] = {"bytes": len(body), "sha256": hashlib.sha256(body).hexdigest()}
            report["prompt"] = {
                "found": bool(prompt),
                "lines": len(prompt.splitlines()),
                "sha256": hashlib.sha256(prompt.encode("utf-8")).hexdigest(),
                "numbers": prompt_numbers,
            }
            for task in report["tasks"]:
                task["numbers_also_in_prompt"] = also_in_prompt(task["asked"], task["expected"], prompt_numbers)
        except Exception as problem:  # noqa: BLE001 - the sample still runs without this record
            report["problems"].append("page instructions not read: " + repr(problem)[:200])

        stopped_early = False
        for repeat in range(1, REPEATS + 1):
            for name, asked, expected in tasks:
                if time.time() - started > BUDGET_MINUTES * 60:
                    stopped_early = True
                    break
                run = run_twice(context, repeat, name, asked, expected)
                report["runs"].append(run)
                save()
                print(
                    f"{name} #{repeat}: {run['outcome']}"
                    f" ({run.get('tries', 0)} tries, {run.get('seconds', 0)} s,"
                    f" models {run.get('models', [])})",
                    flush=True,
                )
                if len(report["runs"]) == 3 and all(
                    one["outcome"] == "harness_error" for one in report["runs"]
                ):
                    report["problems"].append("the first three runs all broke in this script, so it stopped")
                    stopped_early = True
                    break
                time.sleep(PAUSE_SECONDS)
            if stopped_early:
                break
        report["stopped_early"] = stopped_early
        context.close()
    except Exception as problem:  # noqa: BLE001 - record it and finish
        report["problems"].append("run: " + repr(problem)[:300])
        report["trace"] = traceback.format_exc()[-1200:]
    finally:
        if browser is not None:
            browser.close()
        playwright.stop()

    report["finished"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    save()

    print()
    for name, _asked, _expected in tasks:
        outcomes = [run["outcome"] for run in report["runs"] if run["task"] == name]
        print(f"{name:18s} " + ", ".join(outcomes))
    print()
    for outcome in OUTCOMES:
        print(f"{outcome:14s} {report['counts'].get(outcome, 0)}")
    for problem in report["problems"]:
        print("PROBLEM " + problem)
    print(f"{report['total']} of {report['planned']} planned runs were made; the report is {report_path}")
    reached_the_page = [run for run in report["runs"] if run["outcome"] != "harness_error"]
    return 0 if reached_the_page else 1


if __name__ == "__main__":
    sys.exit(main())
