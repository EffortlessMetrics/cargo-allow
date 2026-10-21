#!/usr/bin/env python3
"""Compiler-free, in-memory falsifiers for the production coverage consumer."""
import copy
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("pages", Path(__file__).with_name("qualify-worklist-pages.py"))
pages = importlib.util.module_from_spec(spec)
spec.loader.exec_module(pages)


class MemoryPath:
    """Only the retained-file operations the actual CI admission seam needs."""
    def __init__(self, files, path=""):
        self.files, self.path = files, Path(path)

    def __truediv__(self, name):
        return MemoryPath(self.files, self.path / name)

    @property
    def parent(self):
        return MemoryPath(self.files, self.path.parent)

    def is_symlink(self):
        return False

    def resolve(self):
        return self.path

    def read_bytes(self):
        return self.files[self.path.as_posix()]


def fixture(total=693, probe=650):
    items = [{"id": f"work-review-due-{index:04d}", "item_kind": "review_due", "status": "review_due",
              "path": f"src/{index}.rs", "risk": "medium", "difficulty": "medium", "message": "x" * 1100}
             for index in range(total)]
    if probe is not None:
        items[probe].update(path=pages.PROBE, item_kind="new_unreceipted_finding", status="new")
    filters = dict.fromkeys("kind family item_kind status allow_id path source_package owner classification "
                            "baseline_debt broad_scope risk difficulty missing_evidence broken_evidence weak_evidence".split())
    full = {"schema_id": "cargo-allow.worklist.v1", "schema_version": 1, "command": "worklist", "tool": "cargo-allow",
            "inventory": {"source": "git_tracked", "root": "/repo"}, "filters": filters,
            "summary": pages.summary(items), "work_items": items}
    files = {"full.json": pages.encoded(full)}
    invocation = {"binary": "/bin/cargo-allow", "root": "/repo", "cwd": "/repo/target/pages"}
    def row(name, offset=None):
        argv = [invocation["binary"], "worklist", "--format", "json"]
        if offset is not None:
            argv += ["--limit", "25", "--offset", str(offset)]
        argv += ["--output", name, "--root", invocation["root"]]
        return {"path": name, "sha256": pages.digest(files[name]), "bytes": len(files[name]), "elapsed_ms": 7,
                "exit_code": 0, "argv": argv}
    rows = []
    for offset in range(0, max(1, total), 25):
        page = copy.deepcopy(full)
        page["work_items"] = items[offset:offset + 25]
        page["summary"] = pages.summary(page["work_items"])
        page["paging"] = {"limit": 25, "offset": offset, "total": total, "unfiltered_total": total}
        name = f"page-{offset:06d}.json"
        files[name] = pages.encoded(page)
        rows.append(row(name, offset))
    source = {"head": "a" * 40, "parent": "e" * 40, "tracked_sha256": "b" * 64, "policy_sha256": "c" * 64,
              "binary_sha256": "d" * 64, "utc_day": "2026-10-21"}
    routed = probe // 25 + 1 if probe is not None else 1
    record = {"schema_id": pages.SCHEMA, "schema_version": 1, "result": "passed", "limit": 25, "byte_cap": 524288,
              "ceiling_ms": 60000, "source_before": source, "source_after": copy.deepcopy(source),
              "invocation": invocation, "full": row("full.json"), "pages": rows, "total": total,
              "counts": {field: dict(pages.Counter(item[field] for item in items)) for field in ("status", "item_kind")},
              "sample_page": rows[routed - 1]["path"], "sample_elapsed_ms": routed * 7,
              "routing_probe": pages.PROBE if probe is not None else None, "routing_pages": routed,
              "traversal_elapsed_ms": len(rows) * 7, "qualification_elapsed_ms": len(rows) * 7 + 7}
    return record, files


def operator_fixture(name, elapsed=1):
    """Synthetic retained inputs shared with the actual CI consumer fixture."""
    assert name in ("worklist", "agent_loop_worklist")
    assert type(elapsed) is int and 0 <= elapsed <= 60000
    agent = name == "agent_loop_worklist"
    record, files = fixture(26, 25 if agent else None)
    for source in (record["source_before"], record["source_after"]):
        source.update(head=("b" if agent else "a") * 40, parent="a" * 40, binary_sha256="0" * 64)
    for row in [record["full"], *record["pages"]]:
        row["elapsed_ms"] = 0
    record["pages"][record["routing_pages"] - 1]["elapsed_ms"] = elapsed
    record.update(sample_elapsed_ms=elapsed, traversal_elapsed_ms=elapsed, qualification_elapsed_ms=elapsed)
    files["coverage.json"] = pages.encoded(record)
    selected = record["sample_page"]
    prefix = f"artifacts/{name}-pages/"
    sample = {"name": name, "phase": "agent_loop" if agent else "targeted", "cache_mode": "not_applicable",
              "status": "passed", "elapsed_ms": elapsed,
              "argv": ["qualify-worklist-pages", "--limit", "25", "--routing-probe", pages.PROBE if agent else "none"],
              "artifact": {"path": prefix + selected, "sha256": pages.digest(files[selected])},
              "semantic_artifact": {"path": prefix + "coverage.json", "sha256": pages.digest(files["coverage.json"])},
              "payload_bytes": len(files[selected]), "semantic_payload_bytes": len(files["coverage.json"])}
    return sample, {prefix + path: data for path, data in files.items()}


class CoverageTests(unittest.TestCase):
    def admit(self, record, files):
        return pages.validate_coverage(pages.encoded(record), files.__getitem__)

    def test_oversized_full_population_and_late_probe_are_preserved(self):
        record, files = fixture()
        self.assertGreater(len(files["full.json"]), pages.CAP)
        self.assertEqual(record["total"], 693)
        self.assertEqual(record["routing_pages"], 27)
        self.assertEqual(self.admit(record, files), record)

    def test_empty_and_exact_page_boundaries(self):
        for size in (0, 1, 25, 26, 50):
            with self.subTest(size=size):
                self.admit(*fixture(size, None))

    def test_shared_operator_fixtures_and_unavailable_parent(self):
        for name in ("worklist", "agent_loop_worklist"):
            for elapsed in (0, 1, 60000):
                with self.subTest(name=name, elapsed=elapsed):
                    sample, files = operator_fixture(name, elapsed)
                    root = MemoryPath(files)
                    pages.validate_sample(sample, root, 60000, "0" * 64, "a" * 40)
                    path = sample["semantic_artifact"]["path"]
                    record = pages.decode(files[path])
                    for source in (record["source_before"], record["source_after"]):
                        source["parent"] = None
                    files[path] = pages.encoded(record)
                    sample["semantic_artifact"]["sha256"] = pages.digest(files[path])
                    sample["semantic_payload_bytes"] = len(files[path])
                    if name == "worklist":
                        pages.validate_sample(sample, root, 60000, "0" * 64, "a" * 40)
                    else:
                        with self.assertRaisesRegex(ValueError, "source commit drift"):
                            pages.validate_sample(sample, root, 60000, "0" * 64, "a" * 40)

    def test_windows_report_paths_preserve_actual_command_spelling(self):
        for native, report in ((r"C:\repo", "C:/repo"), (r"\\?\C:\repo", "C:/repo"),
                               (r"\\server\share\repo", "//server/share/repo"),
                               (r"\\?\UNC\server\share\repo", "//server/share/repo")):
            record, files = fixture(26, None)
            record["invocation"]["root"] = native
            for row in [record["full"], *record["pages"]]:
                row["argv"][-1] = native
                document = pages.decode(files[row["path"]])
                document["inventory"]["root"] = report
                files[row["path"]] = pages.encoded(document)
                row.update(sha256=pages.digest(files[row["path"]]), bytes=len(files[row["path"]]))
            with self.subTest(native=native):
                self.admit(record, files)

    def test_member_hash_and_last_page_are_required(self):
        for change in ("hash", "bytes", "missing", "extra", "path", "argv", "exit"):
            record, files = fixture()
            if change == "hash":
                record["pages"][0]["sha256"] = "0" * 64
            elif change == "bytes":
                record["pages"][0]["bytes"] += 1
            elif change == "missing":
                record["pages"].pop()
            elif change == "extra":
                record["pages"].append(record["pages"][-1])
            elif change == "path":
                record["pages"][0]["path"] = "../full.json"
            elif change == "argv":
                record["pages"][0]["argv"] += ["--status", "new"]
            else:
                record["pages"][0]["exit_code"] = True
            with self.subTest(change=change), self.assertRaises(ValueError):
                self.admit(record, files)

    def test_forged_page_semantics_refuse_even_with_recomputed_hash(self):
        for change in ("omit", "duplicate", "order", "content", "total", "unfiltered", "offset", "context", "empty", "oversized"):
            record, files = fixture()
            row = record["pages"][0]
            page = pages.decode(files[row["path"]])
            if change == "omit":
                page["work_items"].pop()
            elif change == "duplicate":
                page["work_items"][1] = page["work_items"][0]
            elif change == "order":
                page["work_items"].reverse()
            elif change == "content":
                page["work_items"][0]["message"] = "different"
            elif change in ("total", "offset"):
                page["paging"][change] += 1
            elif change == "unfiltered":
                page["paging"]["unfiltered_total"] += 1
            elif change == "context":
                page["inventory"]["root"] = "/other"
            elif change == "empty":
                page["work_items"] = []
            else:
                page["work_items"][0]["message"] = "x" * pages.CAP
            page["summary"] = pages.summary(page["work_items"])
            files[row["path"]] = pages.encoded(page)
            row.update(sha256=pages.digest(files[row["path"]]), bytes=len(files[row["path"]]))
            with self.subTest(change=change), self.assertRaises(ValueError):
                self.admit(record, files)

    def test_source_date_identity_and_latency_drift_refuse(self):
        for change in ("utc_day", "tracked_sha256", "policy_sha256", "head", "binary_sha256", "routing", "latency", "traversal", "qualification", "cap"):
            record, files = fixture()
            if change in record["source_after"]:
                record["source_after"][change] = "changed"
            elif change == "routing":
                record["routing_pages"] = 1
            elif change == "latency":
                record["sample_elapsed_ms"] = 7
            elif change == "traversal":
                record["traversal_elapsed_ms"] = 0
            elif change == "qualification":
                record["qualification_elapsed_ms"] = 0
            else:
                record["byte_cap"] += 1
            with self.subTest(change=change), self.assertRaises(ValueError):
                self.admit(record, files)

    def test_probe_must_really_exist_and_companion_must_fit(self):
        record, files = fixture(probe=None)
        record["routing_probe"] = pages.PROBE
        with self.assertRaises(ValueError):
            self.admit(record, files)
        record, files = fixture()
        with self.assertRaises(ValueError):
            pages.validate_coverage(pages.encoded(record) + b" " * pages.CAP, files.__getitem__)
        with self.assertRaises(ValueError):
            pages.decode(b'{"total":1,"total":2}')
        with self.assertRaises(ValueError):
            pages.decode(b'{"total":NaN}')

    def test_actual_ci_sample_admission_binds_page_companion_binary_and_parent(self):
        record, files = fixture()
        files["coverage.json"] = pages.encoded(record)
        selected = record["sample_page"]
        sample = {"name": "agent_loop_worklist", "elapsed_ms": record["sample_elapsed_ms"],
                  "argv": ["qualify-worklist-pages", "--limit", "25", "--routing-probe", pages.PROBE],
                  "artifact": {"path": selected, "sha256": pages.digest(files[selected])},
                  "semantic_artifact": {"path": "coverage.json", "sha256": pages.digest(files["coverage.json"])},
                  "payload_bytes": len(files[selected]), "semantic_payload_bytes": len(files["coverage.json"])}
        root = MemoryPath(files)
        pages.validate_sample(sample, root, 60000, "d" * 64, "e" * 40)
        for field in ("elapsed_ms", "payload_bytes", "semantic_payload_bytes"):
            changed = copy.deepcopy(sample)
            changed[field] += 1
            with self.subTest(field=field), self.assertRaises(ValueError):
                pages.validate_sample(changed, root, 60000, "d" * 64, "e" * 40)
        for binary, head in (("0" * 64, "e" * 40), ("d" * 64, "a" * 40)):
            with self.assertRaises(ValueError):
                pages.validate_sample(sample, root, 60000, binary, head)
        changed = copy.deepcopy(sample)
        changed["artifact"]["path"] = "page-000000.json"
        with self.assertRaises(ValueError):
            pages.validate_sample(changed, root, 60000, "d" * 64, "e" * 40)


if __name__ == "__main__":
    unittest.main()
