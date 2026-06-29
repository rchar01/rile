#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
# SPDX-License-Identifier: GPL-3.0-or-later

import argparse
import fcntl
import json
import os
import pathlib
import pty
import re
import select
import shutil
import signal
import statistics
import struct
import subprocess
import tempfile
import termios
import time


ROOT = pathlib.Path("/workspace")
ARTIFACT_ROOT = ROOT / "artifacts" / "perf"
COLUMNS = 100
ROWS = 24

CTRL_X_CTRL_C = b"\x18\x03"
CTRL_A = b"\x01"
CTRL_E = b"\x05"
CTRL_V = b"\x16"
META_V = b"\x1bv"


def main():
    parser = argparse.ArgumentParser(description="Run optional editor perf smoke tests.")
    parser.add_argument(
        "--editors",
        default=os.environ.get("PERF_EDITORS", "rile,emacs,zile,kg,vi"),
        help="comma-separated editors to run: rile,emacs,zile,kg,vi",
    )
    parser.add_argument(
        "--level",
        choices=("smoke", "full"),
        default=os.environ.get("PERF_LEVEL", "smoke"),
        help="fixture size level",
    )
    parser.add_argument(
        "--repetitions",
        type=int,
        default=int(os.environ.get("PERF_REPETITIONS", "3")),
        help="number of repetitions per editor/case",
    )
    args = parser.parse_args()

    run_id = time.strftime("%Y%m%d-%H%M%S")
    run_dir = ARTIFACT_ROOT / "runs" / run_id
    fixture_dir = ARTIFACT_ROOT / "fixtures" / args.level
    time_dir = run_dir / "time"
    home_dir = run_dir / "home"
    run_dir.mkdir(parents=True, exist_ok=True)
    fixture_dir.mkdir(parents=True, exist_ok=True)
    time_dir.mkdir(parents=True, exist_ok=True)
    home_dir.mkdir(parents=True, exist_ok=True)

    prepare_fixtures(fixture_dir, args.level)
    editors = build_editors()
    requested = [name.strip() for name in args.editors.split(",") if name.strip()]
    unknown = [name for name in requested if name not in editors]
    if unknown:
        raise SystemExit(f"unknown editor(s): {', '.join(unknown)}")

    cases = build_cases(fixture_dir, args.level)
    records = []
    for editor_name in requested:
        editor = editors[editor_name]
        for case in cases:
            for repetition in range(1, args.repetitions + 1):
                record = run_case(
                    editor, case, repetition, time_dir, home_dir, run_dir / "pty"
                )
                records.append(record)
                print(json.dumps(record, sort_keys=True), flush=True)

    results_path = run_dir / "results.jsonl"
    with results_path.open("w", encoding="utf-8") as results:
        for record in records:
            results.write(json.dumps(record, sort_keys=True) + "\n")

    summary_path = run_dir / "summary.md"
    write_summary(summary_path, records, args)
    print(f"\nPerf summary: {summary_path}")
    print(f"Perf results: {results_path}")


def prepare_fixtures(fixture_dir, level):
    write_many_lines(fixture_dir / "many-lines-50mb.txt", 500_000)
    write_long_line(fixture_dir / "long-line-100k.txt", 100_000)
    write_scroll_file(fixture_dir / "scroll-100k-lines.txt", 100_000)
    if level == "full":
        write_many_lines(fixture_dir / "many-lines-200mb.txt", 2_000_000)
        write_long_line(fixture_dir / "long-line-1m.txt", 1_000_000)


def write_many_lines(path, lines):
    expected_size = lines * 104
    if path.exists() and path.stat().st_size == expected_size:
        return
    filler = "x" * 80
    with path.open("w", encoding="utf-8", newline="\n") as fixture:
        for line in range(lines):
            fixture.write(f"large file line {line:06} {filler}\n")


def write_long_line(path, columns):
    if path.exists() and path.stat().st_size >= columns:
        return
    start = "LONG_LINE_START "
    end = " LONG_LINE_END"
    middle = "x" * max(0, columns - len(start) - len(end))
    path.write_text(f"{start}{middle}{end}\nshort tail\n", encoding="utf-8")


def write_scroll_file(path, lines):
    if path.exists() and path.stat().st_size > lines * 32:
        return
    filler = "scroll fixture text"
    with path.open("w", encoding="utf-8", newline="\n") as fixture:
        for line in range(lines):
            fixture.write(f"scroll line {line:06} {filler}\n")


def build_cases(fixture_dir, level):
    cases = [
        {
            "name": "many-lines-50mb-open",
            "file": fixture_dir / "many-lines-50mb.txt",
            "wait": "large file line 000000",
            "operation": None,
            "timeout": 15,
        },
        {
            "name": "long-line-100k-open",
            "file": fixture_dir / "long-line-100k.txt",
            "wait": "LONG_LINE_START",
            "operation": None,
            "timeout": 15,
        },
        {
            "name": "long-line-100k-end",
            "file": fixture_dir / "long-line-100k.txt",
            "wait": "LONG_LINE_START",
            "operation": "end_of_line",
            "operation_wait": "LINE_END",
            "timeout": 20,
        },
        {
            "name": "scroll-100k-lines-page-burst",
            "file": fixture_dir / "scroll-100k-lines.txt",
            "wait": "scroll line 000000",
            "operation": "page_down_burst",
            "timeout": 15,
        },
    ]
    if level == "full":
        cases.extend(
            [
                {
                    "name": "many-lines-200mb-open",
                    "file": fixture_dir / "many-lines-200mb.txt",
                    "wait": "large file line 000000",
                    "operation": None,
                    "timeout": 45,
                },
                {
                    "name": "long-line-1m-end",
                    "file": fixture_dir / "long-line-1m.txt",
                    "wait": "LONG_LINE_START",
                    "operation": "end_of_line",
                    "operation_wait": "LINE_END",
                    "timeout": 45,
                },
            ]
        )
    return cases


def build_editors():
    vi = shutil.which("vi") or "/usr/bin/vi"
    return {
        "rile": {
            "name": "rile",
            "command": lambda file: [
                str(ROOT / "target" / "release" / "rile"),
                "--visual-test",
                "--test-size",
                f"{COLUMNS}x{ROWS}",
                str(file),
            ],
            "quit": CTRL_X_CTRL_C,
            "end_of_line": CTRL_E,
            "page_down": CTRL_V,
            "redraw": b"\x0c",
        },
        "emacs": {
            "name": "emacs",
            "command": lambda file: [
                str(ROOT / "artifacts" / "reference" / "emacs" / "install" / "bin" / "emacs-core"),
                "--eval",
                "(setq large-file-warning-threshold nil)",
                str(file),
            ],
            "quit": CTRL_X_CTRL_C,
            "end_of_line": CTRL_E,
            "page_down": CTRL_V,
            "redraw": b"\x0c",
        },
        "zile": {
            "name": "zile",
            "command": lambda file: [
                str(ROOT / "artifacts" / "reference" / "zile" / "install" / "bin" / "zile"),
                "--no-init-file",
                str(file),
            ],
            "quit": CTRL_X_CTRL_C,
            "end_of_line": CTRL_E,
            "page_down": CTRL_V,
            "redraw": b"\x0c",
        },
        "kg": {
            "name": "kg",
            "command": lambda file: [
                str(ROOT / "artifacts" / "reference" / "kg" / "install" / "bin" / "kg"),
                str(file),
            ],
            "quit": CTRL_X_CTRL_C,
            "end_of_line": CTRL_E,
            "page_down": CTRL_V,
            "redraw": b"\x0c",
        },
        "vi": {
            "name": "vi",
            "command": lambda file: [vi, "-Nu", "NONE", "-n", "-i", "NONE", str(file)],
            "quit": b"\x1b:q!\r",
            "end_of_line": b"$",
            "page_down": b"\x06",
            "redraw": b"\x0c",
        },
    }


def run_case(editor, case, repetition, time_dir, home_dir, pty_dir):
    label = f"{editor['name']}-{case['name']}-{repetition}"
    time_path = time_dir / f"{label}.txt"
    pty_dir.mkdir(parents=True, exist_ok=True)
    pty_path = pty_dir / f"{label}.log"
    editor_home = home_dir / label
    editor_home.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    env.update(
        {
            "HOME": str(editor_home),
            "TERM": "xterm-256color",
            "NO_COLOR": "1",
            "COLUMNS": str(COLUMNS),
            "LINES": str(ROWS),
        }
    )
    command = ["/usr/bin/time", "-v", "-o", str(time_path)] + editor["command"](case["file"])

    runner = PtyRunner(command, env, case["timeout"])
    started = time.perf_counter()
    record = {
        "editor": editor["name"],
        "case": case["name"],
        "repetition": repetition,
        "file_bytes": case["file"].stat().st_size,
        "status": "ok",
    }
    try:
        runner.wait_for(case["wait"])
        open_elapsed = time.perf_counter() - started
        operation_elapsed = None
        if case.get("operation") == "end_of_line":
            before = runner.bytes_seen()
            operation_started = time.perf_counter()
            runner.send(editor["end_of_line"])
            runner.wait_for(case["operation_wait"], start=before)
            operation_elapsed = time.perf_counter() - operation_started
        elif case.get("operation") == "page_down_burst":
            before = runner.bytes_seen()
            operation_started = time.perf_counter()
            for _ in range(20):
                runner.send(editor["page_down"])
                runner.drain(0.03)
            runner.send(editor["redraw"])
            runner.drain(0.25)
            assert_scroll_advanced(runner.output[before:])
            operation_elapsed = time.perf_counter() - operation_started

        runner.send(editor["quit"])
        runner.wait_exit(5)
        total_elapsed = time.perf_counter() - started
        record.update(
            {
                "open_seconds": round(open_elapsed, 6),
                "operation_seconds": None
                if operation_elapsed is None
                else round(operation_elapsed, 6),
                "total_seconds": round(total_elapsed, 6),
            }
        )
    except Exception as error:
        record.update(
            {
                "status": "failed",
                "error": str(error),
                "pty_log": str(pty_path),
                "output_tail": runner.output[-1000:].decode("utf-8", errors="replace"),
            }
        )
        runner.terminate()
    finally:
        pty_path.write_bytes(bytes(runner.output))
        runner.close()

    time_info = parse_time_file(time_path)
    record.update(time_info)
    exit_status = time_info.get("time_exit_status")
    if exit_status not in (None, 0) and record["status"] == "ok":
        record.update(
            {
                "status": "failed",
                "error": f"editor exited with status {exit_status}",
                "pty_log": str(pty_path),
            }
        )
    return record


class PtyRunner:
    def __init__(self, command, env, timeout):
        self.timeout = timeout
        self.master, slave = pty.openpty()
        set_winsize(slave, ROWS, COLUMNS)
        self.process = subprocess.Popen(
            command,
            cwd=str(ROOT),
            env=env,
            stdin=slave,
            stdout=slave,
            stderr=slave,
            preexec_fn=controlling_tty_setup(slave),
        )
        os.close(slave)
        os.set_blocking(self.master, False)
        self.output = bytearray()

    def bytes_seen(self):
        return len(self.output)

    def wait_for(self, text, start=0):
        needle = text.encode("utf-8")
        deadline = time.monotonic() + self.timeout
        while time.monotonic() < deadline:
            self.drain(0.05)
            if needle in self.output[start:]:
                return
            if self.process.poll() is not None:
                raise RuntimeError(f"process exited before screen contained {text!r}")
        tail = self.output[-2000:].decode("utf-8", errors="replace")
        raise TimeoutError(f"screen did not contain {text!r}; tail={tail!r}")

    def send(self, data):
        os.write(self.master, data)

    def drain(self, seconds):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            readable, _, _ = select.select([self.master], [], [], 0.02)
            if not readable:
                continue
            try:
                chunk = os.read(self.master, 65536)
            except BlockingIOError:
                continue
            except OSError:
                return
            if not chunk:
                return
            self.output.extend(chunk)

    def wait_exit(self, timeout):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            self.drain(0.05)
            if self.process.poll() is not None:
                return
        raise TimeoutError("process did not exit after quit")

    def terminate(self):
        if self.process.poll() is not None:
            return
        try:
            os.killpg(self.process.pid, signal.SIGTERM)
            self.process.wait(timeout=2)
        except Exception:
            try:
                os.killpg(self.process.pid, signal.SIGKILL)
            except Exception:
                pass

    def close(self):
        try:
            os.close(self.master)
        except OSError:
            pass


def set_winsize(fd, rows, columns):
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, columns, 0, 0))


def assert_scroll_advanced(output):
    text = output.decode("utf-8", errors="replace")
    lines = [int(match) for match in re.findall(r"scroll line (\d{6})", text)]
    if not lines or max(lines) < 30:
        raise RuntimeError("page-down burst did not render later scroll fixture lines")


def controlling_tty_setup(slave_fd):
    def setup():
        os.setsid()
        fcntl.ioctl(slave_fd, termios.TIOCSCTTY, 0)

    return setup


def parse_time_file(path):
    result = {}
    if not path.exists():
        return result
    text = path.read_text(encoding="utf-8", errors="replace")
    rss = re.search(r"Maximum resident set size \(kbytes\):\s*(\d+)", text)
    status = re.search(r"Exit status:\s*(\d+)", text)
    if rss:
        result["max_rss_kb"] = int(rss.group(1))
    if status:
        result["time_exit_status"] = int(status.group(1))
    return result


def write_summary(path, records, args):
    grouped = {}
    for record in records:
        grouped.setdefault((record["editor"], record["case"]), []).append(record)

    lines = [
        "# Performance Smoke Results",
        "",
        f"- Level: `{args.level}`",
        f"- Repetitions: `{args.repetitions}`",
        f"- Terminal size: `{COLUMNS}x{ROWS}`",
        "",
        "| Editor | Case | Status | Open Median (s) | Operation Median (s) | Max RSS Median (KiB) |",
        "| --- | --- | --- | ---: | ---: | ---: |",
    ]
    for (editor, case), values in sorted(grouped.items()):
        statuses = sorted({value["status"] for value in values})
        open_values = [value["open_seconds"] for value in values if value.get("open_seconds") is not None]
        op_values = [
            value["operation_seconds"]
            for value in values
            if value.get("operation_seconds") is not None
        ]
        rss_values = [value["max_rss_kb"] for value in values if value.get("max_rss_kb") is not None]
        lines.append(
            "| {editor} | {case} | {status} | {open_median} | {op_median} | {rss_median} |".format(
                editor=editor,
                case=case,
                status=", ".join(statuses),
                open_median=format_median(open_values),
                op_median=format_median(op_values),
                rss_median=format_median(rss_values, digits=0),
            )
        )
    lines.append("")
    path.write_text("\n".join(lines), encoding="utf-8")


def format_median(values, digits=6):
    if not values:
        return ""
    value = statistics.median(values)
    if digits == 0:
        return str(int(value))
    return f"{value:.{digits}f}"


if __name__ == "__main__":
    main()
