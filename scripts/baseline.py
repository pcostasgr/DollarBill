#!/usr/bin/env python3
"""Reproduce the offline engineering baseline. Python standard library only."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def category(path):
    if any(s in path for s in ("test_july_replay", "test_kill_switches")):
        return "incident_regression"
    for token, name in (("execution", "execution"), ("alpaca", "broker_contract"),
                        ("persistence", "persistence"), ("risk", "risk"),
                        ("strateg", "strategy"), ("models", "mathematical"),
                        ("calibration", "mathematical"), ("pricing_validation", "mathematical"),
                        ("market_data", "market_data"), ("integration", "integration"),
                        ("backtesting", "backtesting"), ("portfolio", "portfolio")):
        if token in path:
            return name
    return "supporting"


def inventory():
    rows = []
    ignored = set()
    ignored_docs = set()
    for folder in ("src", "tests"):
        for path in sorted((ROOT / folder).rglob("*.rs")):
            relative = path.relative_to(ROOT).as_posix()
            source = path.read_text(encoding="utf-8")
            for index, _ in enumerate(re.finditer(r"(?m)^\s*//[/!]\s*```(?:rust,)?ignore", source), 1):
                ignored_docs.add((relative, index))
            # Annotated source tests, including proptest functions; the test
            # harness logs remain authoritative for runtime case counts.
            pattern = r"(?m)^\s*#\[(?:tokio::)?test\][\s\S]*?\b(?:async\s+)?fn\s+(\w+)"
            for match in re.finditer(pattern, source):
                is_ignored = bool(re.search(r"(?m)^\s*#\[ignore(?:\s*=.*?)?\]", match.group()))
                row = {"path": relative, "name": match[1], "category": category(relative), "ignored": is_ignored}
                rows.append(row)
                if is_ignored:
                    ignored.add((relative, match[1]))
    exceptions = json.loads((ROOT / "docs/ignored-tests.json").read_text(encoding="utf-8"))
    documented = {(e["path"], e["name"]) for e in exceptions["exceptions"]}
    if ignored != documented:
        raise RuntimeError(f"Ignored-test inventory mismatch: undocumented={ignored - documented}, stale={documented - ignored}")
    if any(not e["reason"].strip() for e in exceptions["exceptions"]):
        raise RuntimeError("Every ignored test needs a reason")
    documented_docs = {(e["path"], e["index"]) for e in exceptions["doctest_exceptions"]}
    if ignored_docs != documented_docs:
        raise RuntimeError(f"Ignored doctest inventory mismatch: {ignored_docs ^ documented_docs}")
    return rows, exceptions


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", default="target/baseline", help="New output directory")
    parser.add_argument("--offline", action="store_true", help="Use Cargo's cached dependencies only")
    args = parser.parse_args()
    output = Path(args.output)
    if not output.is_absolute():
        output = ROOT / output
    output = output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    checks = []
    manifest = {"schema_version": 1, "status": "running", "checks": checks,
                "baseline_script_sha256": digest(Path(__file__)),
                "deferred_gates": ["corrected historical strategy matrices", "benchmark archival",
                                   "QuantLib validation", "ignored numerical gates", "live adapter recovery"]}

    def write_manifest():
        (output / "baseline.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")

    def run(name, command):
        print(f"[{name}] {' '.join(map(str, command))}", flush=True)
        # Never echo credentials; none of these commands submits broker orders.
        with (output / f"{name}.log").open("w", encoding="utf-8") as log:
            process = subprocess.run(command, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT,
                                     text=True, encoding="utf-8", errors="replace")
        checks.append({"name": name, "command": list(map(str, command)), "exit_code": process.returncode})
        write_manifest()
        if process.returncode:
            raise RuntimeError(f"{name} failed; see {output / (name + '.log')}")
        return (output / f"{name}.log").read_text(encoding="utf-8")

    try:
        rows, exceptions = inventory()
        (output / "test_inventory.json").write_text(json.dumps(rows, indent=2) + "\n", encoding="utf-8")
        (output / "ignored_tests.json").write_text(json.dumps(exceptions, indent=2) + "\n", encoding="utf-8")
        flags = ["--locked"] + (["--offline"] if args.offline else [])
        run("check", ["cargo", "check", *flags, "--all-targets"])
        run("tests", ["cargo", "test", *flags, "--lib", "--tests", "--", "--quiet"])
        run("doctests", ["cargo", "test", *flags, "--doc"])
        run("ignored", ["cargo", "test", *flags, "--lib", "--tests", "--", "--ignored", "--list"])
        run("build_replay", ["cargo", "build", *flags, "--bin", "dollarbill-replay"])
        # Ask Cargo for the target directory rather than assume CARGO_TARGET_DIR.
        metadata = subprocess.check_output(["cargo", "metadata", *flags, "--no-deps", "--format-version", "1"], cwd=ROOT, text=True)
        binary = Path(json.loads(metadata)["target_directory"]) / "debug" / ("dollarbill-replay.exe" if os.name == "nt" else "dollarbill-replay")
        manifest["build"] = json.loads(run("build_info", [str(binary), "build-info"]))
        inputs = sorted((ROOT / "tests/fixtures/execution").glob("*.json"))
        if not inputs:
            raise RuntimeError("No deterministic scenarios found")
        manifest["inputs"] = {p.relative_to(ROOT).as_posix(): digest(p) for p in inputs}
        manifest["cargo_lock_sha256"] = digest(ROOT / "Cargo.lock")
        for path in inputs:
            name = path.stem
            first = output / name
            second = output / (name + "_repeat")
            for destination in (first, second):
                run(destination.name, [str(binary), "simulate", "--scenario", str(path), "--output", str(destination)])
            for artifact in ("events.jsonl", "state.json", "run.json", "orders.json", "fills.json", "positions.json", "metrics.json"):
                if (first / artifact).read_bytes() != (second / artifact).read_bytes():
                    raise RuntimeError(f"Non-deterministic {name}/{artifact}")
            state = json.loads(run(name + "_replay", [str(binary), "replay", "--events", str(first / "events.jsonl")]))
            if state != json.loads((first / "state.json").read_text(encoding="utf-8")):
                raise RuntimeError(f"Replay divergence in {name}")
        manifest["artifacts"] = {p.relative_to(output).as_posix(): digest(p)
                                 for p in sorted(output.rglob("*")) if p.is_file() and p.name != "baseline.json"}
        manifest["status"] = "passed"
        write_manifest()
        print(f"Baseline passed: {output / 'baseline.json'}")
    except Exception as error:
        manifest["status"] = "failed"
        manifest["error"] = str(error)
        write_manifest()
        raise


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
