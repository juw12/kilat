"""Open the live GlowCode page in a real browser and run Kilat programs through it.

    python tests/glowcode_live_test.py <report.json>

GlowCode (https://juw12.github.io/projects/glow/code/) is the code page of the Glow AI app. It ships
Kilat's web engine, kilat.js and kilat.wasm. This test uses the page the way a person does: it taps
each of the five example chips, types four more programs, presses Run and reads what the page shows.
It exits 1 when the page reports another Kilat version than expected or a case does not behave as
stated below.

It needs Playwright:  pip install playwright   then   python -m playwright install chromium

Environment:
    KILAT_PAGE_URL      the page to open (default: the live page)
    KILAT_PAGE_VERSION  the Kilat version the page must report (default: the one in Cargo.toml)
"""

import hashlib
import json
import os
import re
import sys
import time
import traceback
from pathlib import Path

from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parent.parent
PAGE_URL = os.environ.get("KILAT_PAGE_URL", "https://juw12.github.io/projects/glow/code/")
LOAD_SECONDS = 120
RUN_SECONDS = 120

# The page's own examples in the order of its chips:
# (chip label, the kind the page must report, texts that must be on the page afterwards).
EXAMPLES = [
    ("Stress in a bar", "ok", ["stress = 20.8 MPa"]),
    ("Gap statistics", "ok", ["mean   2.10 mm", "median 2.05 mm", "spread 0.216 mm"]),
    ("A unit mistake", "compile", ["cannot add m and kg"]),
    ("A function and a loop", "ok", ["0\n1\n1\n2\n3\n5\n8\n13\n21\n34"]),
    ("A loop that never ends", "fuel", ["work limit"]),
]

# Programs typed into the code box: (name, program, kind, texts that must be on the page).
TYPED = [
    (
        "spring, units inside the braces",
        "let stiffness = 12 N/mm\n"
        "let travel = 8 mm\n"
        "let force = stiffness * travel\n"
        "let energy = stiffness * travel ^ 2 / 2\n"
        'print("force = {force:N}")\n'
        'print("energy = {energy:J.3}")\n',
        "ok",
        ["96.0 N", "0.384 J"],
    ),
    (
        "shaft torque from power / speed with rpm",
        "let power = 7.5 kW\n"
        "let speed = 1450 rpm\n"
        'print("torque = {power / speed:N*m.1}")\n',
        "ok",
        ["49.4 N*m"],
    ),
    (
        "rpm times 2 * pi / 60 is refused",
        "let power = 7.5 kW\n"
        "let speed = 1450 rpm\n"
        "let omega = speed * 2 * pi / 60\n"
        'print("torque = {power / omega:N*m.1}")\n',
        "compile",
        ["rpm"],
    ),
    (
        "unit word after a placeholder is refused",
        "let force = 96 N\n"
        'print("force = {force} N")\n',
        "compile",
        ["{force:N}"],
    ),
]

READ_RESULT = """() => {
  const shown = (id) => {
    const node = document.getElementById(id);
    return node && !node.hidden ? node.textContent : '';
  };
  return {
    kind: document.getElementById('result').dataset.kind || '',
    status: document.getElementById('result-status').textContent,
    output: shown('result-output'),
    error: shown('result-error'),
  };
}"""


def repo_version():
    text = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    found = re.search(r'^version\s*=\s*"([^"]+)"', text, re.MULTILINE)
    return found.group(1) if found else ""


def file_facts(data):
    return {"bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}


def open_page(context):
    """A fresh page with the engine loaded and the "I write it" mode chosen."""
    page = context.new_page()
    page.goto(PAGE_URL, wait_until="domcontentloaded", timeout=60000)
    page.wait_for_function(
        "() => ['ready', 'failed'].includes(document.body.dataset.run)",
        timeout=LOAD_SECONDS * 1000,
    )
    page.click('#brain-seg button[data-brain="rules"]')
    return page


def press_run(page):
    page.wait_for_function("() => !document.getElementById('run-btn').disabled", timeout=60000)
    page.click("#run-btn", timeout=60000)
    page.wait_for_function("() => document.body.dataset.run === 'done'", timeout=RUN_SECONDS * 1000)
    return page.evaluate(READ_RESULT)


def judge(shown, kind, must_show):
    """The reasons a result is not the stated one; an empty list means it is."""
    reasons = []
    if shown["kind"] != kind:
        reasons.append(f"the page reported kind {shown['kind']!r}, expected {kind!r}")
    on_page = shown["status"] + "\n" + shown["output"] + "\n" + shown["error"]
    for wanted in must_show:
        if wanted not in on_page:
            reasons.append(f"{wanted!r} is not on the page")
    return reasons


def run_example(context, position, label):
    page = open_page(context)
    try:
        chip = page.locator("#example-chips button.chip").nth(position)
        chip_text = chip.text_content(timeout=30000) or ""
        chip.click()
        code = page.input_value("#code-box")
        shown = press_run(page)
    finally:
        page.close()
    shown["chip"] = chip_text.strip()
    shown["code"] = code
    if label not in chip_text:
        shown["chip_problem"] = f"chip {position + 1} is {chip_text.strip()!r}, expected {label!r}"
    return shown


def run_typed(context, program):
    page = open_page(context)
    try:
        page.fill("#code-box", program)
        return press_run(page)
    finally:
        page.close()


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
    expected_version = os.environ.get("KILAT_PAGE_VERSION") or repo_version()
    report = {
        "page": PAGE_URL,
        "expected_version": expected_version,
        "started": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "cases": [],
        "problems": [],
    }

    def add(name, kind, must_show, shown):
        reasons = judge(shown, kind, must_show)
        if shown.get("chip_problem"):
            reasons.append(shown["chip_problem"])
        report["cases"].append(
            {"name": name, "expected": kind, "must_show": must_show, "shown": shown, "reasons": reasons}
        )
        print(("ok   " if not reasons else "FAIL ") + name + ("" if not reasons else ": " + "; ".join(reasons)))

    def attempt(name, kind, must_show, job):
        try:
            add(name, kind, must_show, job())
        except Exception as problem:  # noqa: BLE001 - one broken case must not hide the others
            report["cases"].append(
                {
                    "name": name,
                    "expected": kind,
                    "must_show": must_show,
                    "reasons": ["the test itself broke: " + repr(problem)[:300]],
                    "trace": traceback.format_exc()[-800:],
                }
            )
            print("FAIL " + name + ": the test itself broke: " + repr(problem)[:200])

    playwright = sync_playwright().start()
    browser = None
    try:
        browser, channel = start_browser(playwright, report["problems"])
        report["browser"] = {"channel": channel, "version": browser.version}
        report["problems"] = []  # a browser started, so the earlier tries do not matter
        context = browser.new_context(viewport={"width": 1280, "height": 900})

        # What the page says about its engine, and which engine file the browser is given.
        page = context.new_page()
        response = page.goto(PAGE_URL, wait_until="domcontentloaded", timeout=60000)
        page.wait_for_function(
            "() => ['ready', 'failed'].includes(document.body.dataset.run)",
            timeout=LOAD_SECONDS * 1000,
        )
        report["page_status"] = response.status if response else None
        report["page_last_modified"] = (response.headers.get("last-modified") if response else None)
        report["engine_state"] = page.evaluate("() => document.body.dataset.run")
        report["engine_line"] = page.text_content("#code-meta")
        page.close()
        wanted_line = f"Kilat {expected_version} is ready"
        version_ok = wanted_line in (report["engine_line"] or "")
        report["version_ok"] = version_ok
        print(("ok   " if version_ok else "FAIL ") + f"the page says {report['engine_line']!r}")
        if not version_ok:
            report["problems"].append(f"the page does not say {wanted_line!r}")

        live_wasm = context.request.get(PAGE_URL + "kilat.wasm", timeout=60000)
        report["page_wasm"] = file_facts(live_wasm.body()) if live_wasm.ok else {"status": live_wasm.status}
        repo_wasm = ROOT / "docs" / "kilat.wasm"
        if repo_wasm.exists():
            report["repo_wasm"] = file_facts(repo_wasm.read_bytes())
            report["same_engine_file"] = report["page_wasm"] == report["repo_wasm"]

        for position, (label, kind, must_show) in enumerate(EXAMPLES):
            attempt(
                f"example {position + 1}: {label}",
                kind,
                must_show,
                lambda position=position, label=label: run_example(context, position, label),
            )
        for name, program, kind, must_show in TYPED:
            attempt(name, kind, must_show, lambda program=program: run_typed(context, program))
        context.close()
    except Exception as problem:  # noqa: BLE001 - record it and fail
        report["problems"].append("run: " + repr(problem)[:300])
        report["trace"] = traceback.format_exc()[-1200:]
    finally:
        if browser is not None:
            browser.close()
        playwright.stop()

    report["passed"] = sum(1 for case in report["cases"] if not case["reasons"])
    report["total"] = len(report["cases"])
    report["finished"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(json.dumps(report, indent=2), encoding="utf-8")

    for problem in report["problems"]:
        print("PROBLEM " + problem)
    print(f"{report['passed']} of {report['total']} cases passed; the report is {report_path}")
    whole = report["total"] == len(EXAMPLES) + len(TYPED)
    return 0 if whole and report["passed"] == report["total"] and not report["problems"] else 1


if __name__ == "__main__":
    sys.exit(main())
