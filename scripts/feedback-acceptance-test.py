#!/usr/bin/env python3
"""Acceptance verifier and launcher cleanup contracts; only self-created temporary files."""
import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import socket
import sqlite3
import tempfile
from types import SimpleNamespace
import unittest

spec = importlib.util.spec_from_file_location("feedback_acceptance", Path(__file__).with_name("feedback-acceptance.py"))
acceptance = importlib.util.module_from_spec(spec)
spec.loader.exec_module(acceptance)


class LauncherStopTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="rambledesk-feedback-acceptance-")
        self.addCleanup(temporary.cleanup)
        self.directory = Path(temporary.name)
        self.manifest_path = self.directory / "acceptance.json"
        # The fixture is already stopped: use a freshly released loopback port,
        # and exercise the real stop command without signalling any process.
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            port = listener.getsockname()[1]
        self.manifest = {
            "fixture": acceptance.FIXTURE, "directory": str(self.directory),
            "database": str(self.directory / "validation.sqlite3"),
            "stopFile": str(self.directory / "stop-fixture"),
            "tokenFile": str(self.directory / "durable-token.txt"),
            "url": f"http://127.0.0.1:{port}", "status": "stopped", "keep": False,
        }

    def stop(self, log, keep=False):
        acceptance.write_json(self.manifest_path, {**self.manifest, "logFile": str(log), "keep": keep})
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            acceptance.stop(SimpleNamespace(manifest=str(self.manifest_path)))
        return json.loads(output.getvalue())

    def launcher_log(self, **options):
        fd, name = tempfile.mkstemp(prefix="rambledesk-feedback-acceptance-", suffix=".log", **options)
        os.close(fd)
        path = Path(name)
        path.write_text("Only this test created this launcher log.\n", encoding="utf-8")
        self.addCleanup(lambda: path.unlink(missing_ok=True))
        return path

    def test_stop_rejects_an_unrelated_file_without_deleting_it(self):
        victim = self.directory / "unrelated-user-document.txt"
        victim.write_text("Must survive the stop command.", encoding="utf-8")
        with self.assertRaisesRegex(RuntimeError, "[Ll]og"):
            self.stop(victim)
        self.assertEqual(victim.read_text(), "Must survive the stop command.")

    def test_stop_rejects_a_matching_filename_outside_the_launcher_temp_root(self):
        victim = self.launcher_log(dir=self.directory)
        with self.assertRaisesRegex(RuntimeError, "[Ll]og"):
            self.stop(victim)
        self.assertTrue(victim.is_file())

    def test_stop_rejects_an_unrelated_filename_in_the_launcher_temp_root(self):
        fd, name = tempfile.mkstemp(prefix="unrelated-acceptance-test-", suffix=".log")
        os.close(fd)
        victim = Path(name)
        self.addCleanup(lambda: victim.unlink(missing_ok=True))
        with self.assertRaisesRegex(RuntimeError, "[Ll]og"):
            self.stop(victim)
        self.assertTrue(victim.is_file())

    @unittest.skipIf(os.name == "nt", "Creating symlinks can require elevated Windows privileges")
    def test_stop_rejects_a_launcher_log_symlink(self):
        victim = self.directory / "symlink-target.txt"
        victim.write_text("Keep this target.", encoding="utf-8")
        log = self.launcher_log()
        log.unlink()
        log.symlink_to(victim)
        with self.assertRaisesRegex(RuntimeError, "[Ll]og"):
            self.stop(log)
        self.assertTrue(log.is_symlink())
        self.assertEqual(victim.read_text(), "Keep this target.")

    def test_stop_deletes_only_the_generated_log_and_honors_keep(self):
        for keep in (False, True):
            with self.subTest(keep=keep):
                log = self.launcher_log()
                result = self.stop(log, keep=keep)
                self.assertTrue(result["stopped"])
                self.assertEqual(log.exists(), keep)
                self.assertEqual(result["retainedDirectory"], str(self.directory) if keep else None)
                if not keep:
                    self.assertTrue(self.stop(log)["stopped"])


class OfflineVerificationTests(unittest.TestCase):
    def test_committed_wal_is_read_without_changing_database_contents(self):
        self.verify_offline_snapshot(has_pending_wal=True)

    def test_checkpointed_database_without_nonempty_wal_remains_read_only(self):
        for keep_empty_wal in [False, True]:
            with self.subTest(keep_empty_wal=keep_empty_wal):
                self.verify_offline_snapshot(has_pending_wal=False, keep_empty_wal=keep_empty_wal)

    def verify_offline_snapshot(self, has_pending_wal, keep_empty_wal=False):
        with tempfile.TemporaryDirectory(prefix="rambledesk-feedback-acceptance-") as name:
            directory = Path(name)
            path = directory / "validation.sqlite3"
            source = directory / "source.sqlite3"
            with contextlib.closing(sqlite3.connect(source)) as writer:
                writer.execute("PRAGMA journal_mode=WAL")
                writer.execute("PRAGMA wal_autocheckpoint=0")
                writer.executescript("""
                    CREATE TABLE feedback_requests (id TEXT, status TEXT, revision INTEGER, resolution TEXT);
                    CREATE TABLE drafts (request_id TEXT, revision INTEGER, document_json TEXT, body_markdown TEXT);
                    CREATE TABLE feedback_results (request_id TEXT);
                    CREATE TABLE feedback_deliveries (request_id TEXT);
                """)
                old = json.dumps({"schemaVersion": 2, "doc": {"type": "doc", "content": []}})
                new = json.dumps({"schemaVersion": 2, "doc": {"type": "doc", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "Committed in WAL"}]}]}})
                writer.execute("INSERT INTO feedback_requests VALUES ('request', 'in_progress', 1, NULL)")
                writer.execute("INSERT INTO drafts VALUES ('request', 1, ?, '')", (old,))
                writer.commit()
                writer.execute("PRAGMA wal_checkpoint(TRUNCATE)")
                writer.execute("UPDATE drafts SET revision=2, document_json=?, body_markdown='Committed in WAL'", (new,))
                writer.execute("UPDATE feedback_requests SET revision=2")
                writer.commit()
                if not has_pending_wal:
                    writer.execute("PRAGMA wal_checkpoint(TRUNCATE)")
                # Freeze committed SQLite bytes into a separate offline database:
                # no writer ever opens this verification target.
                path.write_bytes(source.read_bytes())
                wal = Path(str(path) + "-wal")
                if has_pending_wal or keep_empty_wal:
                    wal.write_bytes(Path(str(source) + "-wal").read_bytes())
                if has_pending_wal:
                    Path(str(path) + "-shm").write_bytes(Path(str(source) + "-shm").read_bytes())
                    self.assertGreater(wal.stat().st_size, 0)
                else:
                    self.assertTrue(not wal.exists() or wal.stat().st_size == 0)
                before = (path.read_bytes(), wal.read_bytes() if wal.exists() else None)
                manifest = directory / "acceptance.json"
                acceptance.write_json(manifest, {
                    "fixture": acceptance.FIXTURE, "directory": str(directory), "database": str(path),
                    "stopFile": str(directory / "stop-fixture"), "tokenFile": str(directory / "durable-token.txt"),
                    "url": "http://127.0.0.1:12345", "status": "stopped",
                    "requests": [{"requestId": "request", "purpose": "ordinary", "savedRevision": 1,
                                  "documentSha256": acceptance.digest(old.encode()), "markdownSha256": acceptance.digest(b"")}],
                })
                args = SimpleNamespace(manifest=str(manifest), offline=True, allow_unpublished=True,
                                       request_id=None, output=None)
                output = io.StringIO()
                with contextlib.redirect_stdout(output):
                    acceptance.verify(args)
                result = json.loads(output.getvalue())["requests"][0]
                self.assertEqual(result["savedRevision"], 2)
                self.assertEqual(result["documentSha256"], acceptance.digest(new.encode()))
                self.assertEqual((path.read_bytes(), wal.read_bytes() if wal.exists() else None), before)


if __name__ == "__main__":
    unittest.main()
