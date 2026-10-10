#!/usr/bin/env python3
"""Windows first-run catalog smoke with fresh homes and a filtered toolchain.

Builds before isolation, then runs only the catalog test executable with an
allowlisted environment. This is process isolation, not an OS/installer sandbox.
No existing Agent, account, npm config, or RambleDesk data is used by the probe.
"""

import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import time
import uuid


ROOT = Path(__file__).resolve().parents[1]


def test_binary():
    result = subprocess.run(
        ["cargo", "test", "-p", "rambledesk-acp", "--test", "catalog_network",
         "--locked", "--no-run", "--message-format=json"],
        cwd=ROOT, stdout=subprocess.PIPE, text=True,
    )
    binary = None
    for line in result.stdout.splitlines():
        artifact = json.loads(line)
        if artifact.get("reason") == "compiler-message":
            print(artifact["message"].get("rendered", ""), file=sys.stderr, end="")
        if (artifact.get("reason") == "compiler-artifact"
                and artifact.get("target", {}).get("name") == "catalog_network"
                and artifact.get("executable")):
            binary = Path(artifact["executable"]).resolve()
    result.check_returncode()
    if binary:
        return binary
    raise RuntimeError("Cargo did not produce the catalog_network test executable")


def isolated_environment(directory, node=None):
    home = directory / "empty-home"
    tools = directory / "toolchain"
    temporary = directory / "temp"
    for path in (home, tools, temporary):
        path.mkdir(parents=True)
    # Keep only Windows process prerequisites. In particular, inherit no Agent
    # credentials, proxy URLs, npm settings, NVM paths, or user PATH entries.
    environment = {key: value for key, value in os.environ.items()
                   if key.upper() in {"SYSTEMROOT", "WINDIR", "COMSPEC", "PATHEXT",
                                      "PROCESSOR_ARCHITECTURE", "NUMBER_OF_PROCESSORS"}}
    system = Path(os.environ["SystemRoot"])
    paths = [tools, system / "System32", system]
    environment.update({
        "PATH": os.pathsep.join(map(str, paths)),
        "HOME": str(home), "USERPROFILE": str(home),
        "APPDATA": str(home / "roaming"), "LOCALAPPDATA": str(home / "local"),
        "CODEX_HOME": str(home / "codex"),
        "XDG_CONFIG_HOME": str(home / "config"),
        "XDG_DATA_HOME": str(home / "data"),
        "XDG_CACHE_HOME": str(home / "cache"),
        "TEMP": str(temporary), "TMP": str(temporary),
        "NPM_CONFIG_USERCONFIG": str(home / "user.npmrc"),
        "NPM_CONFIG_GLOBALCONFIG": str(home / "global.npmrc"),
        "NPM_CONFIG_PREFIX": str(home / "npm-global"),
        "NPM_CONFIG_CACHE": str(home / "npm-cache"),
        "RAMBLEDESK_ISOLATED_CATALOG_SMOKE": "1",
    })
    for key in ("APPDATA", "LOCALAPPDATA", "CODEX_HOME", "XDG_CONFIG_HOME",
                "XDG_DATA_HOME", "XDG_CACHE_HOME", "NPM_CONFIG_PREFIX", "NPM_CONFIG_CACHE"):
        Path(environment[key]).mkdir(parents=True)
    for key in ("NPM_CONFIG_USERCONFIG", "NPM_CONFIG_GLOBALCONFIG"):
        Path(environment[key]).write_text("", encoding="utf-8")
    if node:
        # A Node install directory can also contain every globally installed
        # Agent. Copy only Node and npm, never put that directory on the PATH.
        npm = node.parent / "node_modules" / "npm"
        if not (npm / "bin" / "npm-cli.js").is_file():
            raise RuntimeError("Expected npm beside node.exe in node_modules/npm")
        shutil.copy2(node, tools / "node.exe")
        shutil.copytree(npm, tools / "node_modules" / "npm")
        (tools / "npm.cmd").write_text(
            '@echo off\n"%~dp0node.exe" "%~dp0node_modules\\npm\\bin\\npm-cli.js" %*\n',
            encoding="utf-8",
        )
    return environment


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-binary", type=Path, help="Use an already-built catalog_network test")
    parser.add_argument("--output", type=Path, help="New directory for the nonsecret report and logs")
    args = parser.parse_args()
    if os.name != "nt":
        parser.error("This runner isolates Windows discovery paths; use a disposable VM on other OSes")
    node_path = shutil.which("node")
    if not node_path:
        parser.error("Node.js with npm is required to build the filtered test toolchain")
    binary = args.test_binary.resolve() if args.test_binary else test_binary()
    if not binary.is_file() or not binary.name.startswith("catalog_network-"):
        parser.error("Expected the catalog_network test executable, not the desktop application")
    output = (args.output or ROOT / ".local-artifacts" /
              f"onboarding-isolated-{datetime.datetime.now():%Y%m%d-%H%M%S}-{uuid.uuid4().hex[:8]}").resolve()
    output.mkdir(parents=True, exist_ok=False)
    # Keep evidence for failed runs too; no automatic removal of unknown paths.
    # Keep runtime paths short: nested npm optional binaries can exceed Windows'
    # legacy path limit if the evidence directory name is reused as their prefix.
    isolation = Path(tempfile.mkdtemp(prefix="rd-"))
    report = {
        "schema_version": 1, "status": "running",
        "scope": "Windows process isolation; no installer, GUI, login, or model validation",
        "commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "working_tree": subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True).splitlines(),
        "test_binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "build": "existing executable" if args.test_binary else "cargo test --locked --no-run",
        "source_files": {name: hashlib.sha256((ROOT / name).read_bytes()).hexdigest() for name in [
            "scripts/onboarding-isolated-smoke.py", "crates/rambledesk-acp/tests/catalog_network.rs", "Cargo.lock"]},
        "isolation_directory": str(isolation), "cases": [],
    }
    try:
        for name, test, node in [
            ("missing", "isolated_missing_runtime_reports_setup_requirements", None),
            ("node", "real_catalog_install_inspect_and_initialize", Path(node_path).resolve()),
        ]:
            started = time.monotonic()
            environment = isolated_environment(isolation / name, node)
            case = {"name": name, "test": test, "status": "running"}
            report["cases"].append(case)
            print(f"Running {name}; evidence: {output}", flush=True)
            log = output / f"{name}.log"
            with log.open("w", encoding="utf-8") as stream:
                result = subprocess.run(
                    [str(binary), test, "--exact", "--ignored", "--nocapture", "--test-threads=1"],
                    cwd=isolation / name, env=environment, stdout=stream,
                    stderr=subprocess.STDOUT, timeout=900,
                )
            contents = log.read_text(encoding="utf-8")
            evidence = re.search(r"isolated evidence: (.+)", contents)
            if evidence:
                catalog_report = Path(evidence[1].strip()) / "report.json"
                case["install_evidence_directory"] = str(catalog_report.parent)
                if catalog_report.is_file():
                    shutil.copy2(catalog_report, output / "catalog.json")
                    case["catalog_report"] = str(output / "catalog.json")
            # Rust exits successfully when a filter matches zero tests.
            passed = result.returncode == 0 and "1 passed; 0 failed" in contents
            case.update(status="passed" if passed else "failed", exit_code=result.returncode,
                        duration_seconds=round(time.monotonic() - started, 2), log=str(log))
            if not passed:
                raise RuntimeError(f"{name} failed; inspect {log}")
        report["status"] = "passed"
    except KeyboardInterrupt:
        if report["cases"] and report["cases"][-1]["status"] == "running":
            report["cases"][-1]["status"] = "cancelled"
        report.update(status="cancelled", error="Interrupted by the operator")
    except Exception as error:
        if report["cases"] and report["cases"][-1]["status"] == "running":
            report["cases"][-1]["status"] = "failed"
        report.update(status="failed", error=str(error))
        print(str(error), file=sys.stderr)
    finally:
        (output / "report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        print(f"Report: {output / 'report.json'}", flush=True)
    return 0 if report["status"] == "passed" else 130 if report["status"] == "cancelled" else 1


if __name__ == "__main__":
    sys.exit(main())
