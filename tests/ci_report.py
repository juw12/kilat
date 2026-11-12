"""Prints the measured results of a CI run as notices, so they show on the run page.

Run after judge.py and the fuzz test: python tests/ci_report.py
"""

import json
import platform
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def load(name):
    path = ROOT / name
    if not path.is_file():
        return None
    return json.loads(path.read_text(encoding="utf-8"))


def notice(title, text):
    print(f"::notice title={title}::{text}")


def main():
    where = f"{platform.system()} {platform.machine()}"
    report = load("judge_report.json")
    if report is None:
        notice("judge", f"{where}: judge_report.json was not written")
    else:
        speed = report["speed"]
        bugs = report["bugs"]
        passed = {
            section: f"{sum(bool(ok) for _, ok in report[section])} of {len(report[section])}"
            for section in ("conformance", "library_agrees", "sandbox")
        }
        slower = ", ".join(speed["slower"]) or "none"
        notice(
            "judge",
            f"{where}; {report['kilat']}; {report['python']}; {report['processor']}; "
            f"speed {speed['geometric_mean']:.2f} whole process, {speed['geometric_mean_net']:.2f} net "
            f"({speed['runs']} runs, slower than CPython: {slower}); "
            f"bugs refused {bugs['kilat_refuses']} of {bugs['total']}, mypy {bugs['mypy_strict_catches']}; "
            f"language tests {passed['conformance']}; library {passed['library_agrees']}; "
            f"sandbox {passed['sandbox']}; failures {len(report['failures'])}",
        )
        for failure in report["failures"]:
            notice("judge failure", failure)
    fuzz = load("fuzz_summary.json")
    if fuzz is None:
        notice("fuzz", f"{where}: fuzz_summary.json was not written")
    else:
        notice(
            "fuzz",
            f"{where}; seed {fuzz['seed']}; {fuzz['cases']} cases; {fuzz['failures']} failures; "
            f"{fuzz['skipped']} skipped; {fuzz['total_seconds']} s",
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
