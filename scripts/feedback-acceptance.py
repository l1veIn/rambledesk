#!/usr/bin/env python3
"""Operate the disposable Rust feedback fixture; Python 3 standard library only."""

import argparse
import contextlib
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import socket
import sqlite3
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request


ROOT = Path(__file__).resolve().parents[1]
FIXTURE = "rambledesk-feedback-acceptance-v1"


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def digest(contents):
    return hashlib.sha256(contents).hexdigest()


def read_json(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))


def write_json(path, value):
    Path(path).write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def load_manifest(path):
    manifest = read_json(path)
    require(manifest.get("fixture") == FIXTURE, "Not a feedback acceptance fixture manifest")
    directory = Path(manifest["directory"]).resolve()
    require(directory.name.startswith("rambledesk-feedback-acceptance-"), "Unexpected fixture directory")
    require(Path(path).resolve() == directory / "acceptance.json", "Manifest path mismatch")
    for key, name in [("database", "validation.sqlite3"), ("stopFile", "stop-fixture"), ("tokenFile", "durable-token.txt")]:
        require(Path(manifest[key]).resolve() == directory / name, f"Unexpected {key}")
    if "logFile" in manifest:
        require(isinstance(manifest["logFile"], str), "Unexpected launcher log path")
        log = Path(manifest["logFile"])
        # start() uses mkstemp directly in this root. Resolve the parent to allow
        # platform aliases such as macOS /var -> /private/var, never a file link.
        require(log.is_absolute() and log.parent.resolve() == Path(tempfile.gettempdir()).resolve(),
                "Unexpected launcher log directory")
        require(re.fullmatch(r"rambledesk-feedback-acceptance-[a-z0-9_]{8}\.log", log.name),
                "Unexpected launcher log filename")
        require(not log.is_symlink() and (not log.exists() or log.is_file()),
                "Launcher log must be a regular file, not a symlink")
    url = urllib.parse.urlsplit(manifest["url"])
    require(url.scheme == "http" and url.hostname == "127.0.0.1" and url.port, "Fixture must use IPv4 loopback")
    return manifest


def start(args):
    if not args.no_build:
        subprocess.run(["cargo", "build", "--locked", "-p", "rambledesk-local-server", "--example", "feedback_acceptance"], cwd=ROOT, check=True)
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--no-deps", "--format-version=1"], cwd=ROOT))
    binary = Path(metadata["target_directory"]) / "debug/examples" / ("feedback_acceptance.exe" if os.name == "nt" else "feedback_acceptance")
    require(binary.is_file(), "Fixture binary missing; run start without --no-build")
    env = os.environ.copy()
    env["RAMBLEDESK_FEEDBACK_ACCEPTANCE"] = "1"
    env["RAMBLEDESK_ACCEPTANCE_KEEP"] = "1" if args.keep else "0"
    env["RAMBLEDESK_ACCEPTANCE_DIST"] = str(Path(args.dist).resolve())
    fd, log_name = tempfile.mkstemp(prefix="rambledesk-feedback-acceptance-", suffix=".log")
    with os.fdopen(fd, "wb") as log:
        process = subprocess.Popen([str(binary)], cwd=ROOT, env=env, stdin=subprocess.DEVNULL,
                                   stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
    try:
        deadline = time.monotonic() + 60
        manifest = None
        while time.monotonic() < deadline:
            for line in Path(log_name).read_text(encoding="utf-8", errors="replace").splitlines():
                if line.startswith('{"fixture":'):
                    manifest = json.loads(line)
                    break
            if manifest:
                break
            require(process.poll() is None, f"Fixture exited before readiness; inspect {log_name}")
            time.sleep(0.1)
        require(manifest, f"Fixture did not become ready; inspect {log_name}")
        manifest.update({
            "logFile": log_name,
            "startedAt": datetime.datetime.now(datetime.timezone.utc).isoformat(),
            "sourceHead": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
            "sourceStatus": subprocess.check_output(["git", "status", "--short"], cwd=ROOT, text=True),
            "sourceDiffSha256": digest(subprocess.check_output(["git", "diff", "HEAD"], cwd=ROOT)),
            "fixtureSourceSha256": digest((ROOT / "crates/rambledesk-local-server/examples/feedback_acceptance.rs").read_bytes()),
            "binarySha256": digest(binary.read_bytes()),
            "build": "existing binary (--no-build)" if args.no_build else "cargo build --locked (workspace Cargo.lock)",
        })
        write_json(manifest["manifestFile"], manifest)
        print(json.dumps(manifest, ensure_ascii=False, indent=2))
    except BaseException:
        process.terminate()
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
        raise


class Application:
    def __init__(self, manifest):
        self.origin = manifest["url"]
        self.generation = None
        # Never print either credential. The server writes only a disposable token file.
        token = Path(manifest["tokenFile"]).read_text(encoding="utf-8")
        self.token = self.post("/api/auth/session", None, token)["session_token"]

    def post(self, path, value, token=None):
        headers = {"Origin": self.origin, "Authorization": "Bearer " + (token or self.token),
                   "Content-Type": "application/json"}
        if self.generation:
            headers["X-RambleDesk-Runtime-Generation"] = self.generation
        request = urllib.request.Request(self.origin + path,
            data=None if value is None else json.dumps(value).encode(), method="POST",
            headers=headers)
        with urllib.request.urlopen(request, timeout=20) as response:
            generation = response.headers.get("X-RambleDesk-Runtime-Generation")
            if generation:
                self.generation = generation
            return json.load(response)

    def call(self, operation, value):
        return self.post("/api/application/" + operation, value)


def package_file(directory, relative):
    path = (directory / relative).resolve()
    require(path.is_relative_to(directory.resolve()), "Package manifest path escaped its directory")
    return path


def normalized_manifest(value):
    # The persisted package retains some null fields; the public DTO omits them.
    value = dict(value)
    for key in ["cooking_model", "cancel_reason", "uncooked_markdown", "uncooked_sha256"]:
        value.setdefault(key, None)
    value.setdefault("request_attachments", [])
    value.setdefault("resolution", "feedback_submitted")
    return value


def verify(args):
    manifest = load_manifest(args.manifest)
    if args.offline:
        require(manifest["status"] == "stopped", "Offline verification requires a gracefully stopped --keep fixture")
    app = None if args.offline else Application(manifest)
    evidence = {"fixture": FIXTURE, "sourceHead": manifest.get("sourceHead"), "url": manifest["url"], "requests": []}
    published = 0
    database_path = Path(manifest["database"]).resolve()
    wal_path = Path(str(database_path) + "-wal")
    # Offline callers must have stopped every writer. A retained nonempty WAL
    # still contains durable facts: immutable would silently ignore them.
    # Only an absent/empty WAL permits immutable, for SQLite builds that cannot
    # inspect a checkpointed WAL-mode database without creating fresh sidecars.
    query = "?mode=ro"
    if args.offline and (not wal_path.exists() or wal_path.stat().st_size == 0):
        query += "&immutable=1"
    with contextlib.closing(sqlite3.connect(database_path.as_uri() + query, uri=True)) as database:
        database.row_factory = sqlite3.Row
        for seed in manifest["requests"]:
            request_id = seed["requestId"]
            row = database.execute("SELECT r.status, r.revision AS request_revision, r.resolution, d.revision AS saved_revision, d.document_json, d.body_markdown FROM feedback_requests r JOIN drafts d ON d.request_id = r.id WHERE r.id = ?", (request_id,)).fetchone()
            require(row, f"Missing durable draft: {request_id}")
            document = json.loads(row["document_json"])
            require(document["schemaVersion"] == 2 and document["doc"]["type"] == "doc", "Structured draft missing")
            if row["saved_revision"] == seed["savedRevision"]:
                require(digest(row["document_json"].encode()) == seed["documentSha256"], "Seed document hash mismatch")
                require(digest(row["body_markdown"].encode()) == seed["markdownSha256"], "Seed markdown hash mismatch")
            if app:
                workspace = app.call("getFeedbackWorkspace", {"request_id": request_id})
                require(workspace["request"]["status"] == row["status"], "HTTP/SQLite terminal status mismatch")
                for key in ["document_json", "body_markdown", "saved_revision"]:
                    require(workspace["draft"][key] == row[key], f"HTTP/SQLite draft mismatch: {key}")
            result = database.execute("SELECT * FROM feedback_results WHERE request_id = ?", (request_id,)).fetchone()
            item = {"purpose": seed["purpose"], "requestId": request_id, "status": row["status"], "savedRevision": row["saved_revision"], "documentSha256": digest(row["document_json"].encode())}
            if result:
                published += 1
                directory = Path(result["directory_path"])
                require(directory.resolve().is_relative_to(Path(manifest["directory"]).resolve()), "Package escaped fixture")
                manifest_bytes = Path(result["manifest_path"]).read_bytes()
                package = json.loads(manifest_bytes)
                require(digest(manifest_bytes) == result["manifest_sha256"], "Persisted manifest hash mismatch")
                require(package["request_id"] == request_id, "Package request mismatch")
                require(package["source_revision"] == row["saved_revision"] == package["draft_revision"], "Package source revision mismatch")
                require(row["request_revision"] == row["saved_revision"] + 1, "Terminal aggregate revision mismatch")
                require(package.get("resolution", "feedback_submitted") == row["resolution"], "Package resolution mismatch")
                markdown = package_file(directory, package["feedback_markdown"]).read_bytes()
                require(digest(markdown) == package["feedback_sha256"], "Feedback body hash mismatch")
                if package.get("uncooked_markdown"):
                    uncooked = package_file(directory, package["uncooked_markdown"]).read_bytes()
                    require(digest(uncooked) == package["uncooked_sha256"], "Source body hash mismatch")
                for attachment in package["attachments"] + package.get("request_attachments", []):
                    contents = package_file(directory, attachment["path"]).read_bytes()
                    require(len(contents) == attachment["byte_size"] and digest(contents) == attachment["sha256"], "Attachment hash/size mismatch")
                if app:
                    projection = app.call("readPublishedFeedback", {"request_id": request_id})
                    require(normalized_manifest(projection["manifest"]) == normalized_manifest(package), "HTTP/disk manifest mismatch")
                    require(projection["markdown"].encode() == markdown, "HTTP/disk feedback bytes mismatch")
                if args.request_id == request_id:
                    if args.download:
                        download = read_json(args.download)
                        require(normalized_manifest(download["manifest"]) == normalized_manifest(package), "Browser download manifest mismatch")
                        require(download["markdown"].encode() == markdown, "Browser download body mismatch")
                        if package.get("uncooked_markdown"):
                            require(download.get("uncooked_markdown", "").encode() == uncooked, "Browser download source body mismatch")
                    if args.download_markdown:
                        require(Path(args.download_markdown).read_bytes() == markdown, "Browser download body mismatch")
                    if args.download_manifest:
                        require(normalized_manifest(read_json(args.download_manifest)) == normalized_manifest(package), "Browser download manifest mismatch")
                item.update({"feedbackSha256": package["feedback_sha256"], "manifestSha256": result["manifest_sha256"], "attachments": len(package["attachments"]), "requestAttachments": len(package.get("request_attachments", []))})
            else:
                require(row["status"] not in ("completed", "cancelled"), "Terminal request has no durable package")
                require(row["request_revision"] == row["saved_revision"], "Editable draft revision mismatch")
            deliveries = database.execute("SELECT count(*) FROM feedback_deliveries WHERE request_id = ?", (request_id,)).fetchone()[0]
            require(deliveries == 0, "External fixture unexpectedly enqueued a managed continuation")
            evidence["requests"].append(item)
    require(published or args.allow_unpublished, "No published package; submit a request or use --allow-unpublished for the seed baseline")
    if args.request_id:
        require(any(item["requestId"] == args.request_id and "feedbackSha256" in item for item in evidence["requests"]), "Selected request has no published package")
    evidence["result"] = "passed"
    evidence["verifiedAt"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
    if args.output:
        write_json(args.output, evidence)
    print(json.dumps(evidence, ensure_ascii=False, indent=2))


def smoke(args):
    manifest = load_manifest(args.manifest)
    app = Application(manifest)
    for seed in manifest["requests"]:
        request_id = seed["requestId"]
        workspace = app.call("getFeedbackWorkspace", {"request_id": request_id})
        require(workspace["request"]["status"] not in ("completed", "cancelled"), "Smoke requires a fresh fixture; it mutates all four requests")
        draft = workspace["draft"]
        document = json.loads(draft["document_json"])
        text = "HTTP acceptance edit: latest saved revision must be published."
        document["doc"]["content"].append({"type": "paragraph", "content": [{"type": "text", "text": text}]})
        save = {"request_id": request_id, "expected_revision": draft["saved_revision"],
                "document_json": json.dumps(document, ensure_ascii=False, separators=(",", ":")),
                "body_markdown": draft["body_markdown"] + "\n" + text + "\n"}
        saved = app.call("saveFeedbackDraft", save)
        try:
            app.call("saveFeedbackDraft", {**save, "body_markdown": "Stale client must not overwrite"})
            raise RuntimeError("Stale CAS save unexpectedly succeeded")
        except urllib.error.HTTPError as error:
            require(error.code == 409, f"Expected CAS conflict, received HTTP {error.code}")
            require(json.load(error)["code"] != "RUNTIME_GENERATION_STALE", "CAS check hit runtime admission instead of draft revision")
        if seed["purpose"] == "cancel":
            result = app.call("cancelFeedbackRequest", {"request_id": request_id, "reason": "Isolated acceptance cancellation"})
            require(result["status"] == "cancelled", "Cancellation did not become terminal")
        else:
            command = {"request_id": request_id, "expected_revision": saved["saved_revision"]}
            first = app.call("submitFeedback", command)
            replay = app.call("submitFeedback", command)
            require(first == replay and first["status"] == "completed", "Submit replay changed the terminal result")
    verify(args)


def stop(args):
    manifest = load_manifest(args.manifest)
    if manifest["status"] != "stopped":
        Path(manifest["stopFile"]).touch()
    deadline = time.monotonic() + 20
    while time.monotonic() < deadline:
        if not Path(args.manifest).exists() or read_json(args.manifest)["status"] == "stopped":
            url = urllib.parse.urlsplit(manifest["url"])
            with socket.socket() as probe:
                probe.settimeout(1)
                require(probe.connect_ex((url.hostname, url.port)) != 0, "Fixture still accepts connections")
            if not manifest["keep"] and manifest.get("logFile"):
                Path(manifest["logFile"]).unlink(missing_ok=True)
            print(json.dumps({"stopped": True, "retainedDirectory": manifest["directory"] if manifest["keep"] else None}))
            return
        time.sleep(0.1)
    raise RuntimeError("Fixture did not stop; inspect its log. No unrelated process was signalled.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    launch = commands.add_parser("start", help="Build and start a fresh isolated fixture")
    launch.add_argument("--dist", default=str(ROOT / "apps/desktop/dist"))
    launch.add_argument("--keep", action="store_true", help="Retain DB/packages after graceful stop")
    launch.add_argument("--no-build", action="store_true", help="Use the existing binary; record this limitation")
    launch.set_defaults(handler=start)
    for name, handler in [("verify", verify), ("smoke", smoke)]:
        command = commands.add_parser(name, help="Read-only artifact verification" if name == "verify" else "Mutating HTTP save/CAS/submit/cancel acceptance on a fresh fixture")
        command.add_argument("manifest")
        command.add_argument("--offline", action="store_true", help="Read only SQLite and files after stop")
        command.add_argument("--allow-unpublished", action="store_true")
        command.add_argument("--request-id")
        command.add_argument("--download", help="Actual .rambledesk-feedback.json exported by the browser")
        command.add_argument("--download-markdown")
        command.add_argument("--download-manifest")
        command.add_argument("--output")
        command.set_defaults(handler=handler)
    shutdown = commands.add_parser("stop", help="Request graceful shutdown; remove disposable data unless --keep")
    shutdown.add_argument("manifest")
    shutdown.set_defaults(handler=stop)
    args = parser.parse_args()
    if (getattr(args, "download", None) or getattr(args, "download_markdown", None) or getattr(args, "download_manifest", None)) and not args.request_id:
        parser.error("--request-id is required with a downloaded file")
    args.handler(args)


if __name__ == "__main__":
    try:
        main()
    except (RuntimeError, OSError, ValueError, sqlite3.Error, subprocess.SubprocessError) as error:
        print(f"feedback-acceptance: {error}", file=sys.stderr)
        sys.exit(1)
