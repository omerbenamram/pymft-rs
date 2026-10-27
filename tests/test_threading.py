import concurrent.futures
import itertools
import os
import subprocess
import sys
import sysconfig
from pathlib import Path

import pytest

from mft import PyMftParser

SAMPLE = Path(__file__).parent.parent / "samples" / "MFT"


def test_parser_can_move_between_threads():
    # Parsers and iterators are plain (sendable) classes, so a parser built on one
    # thread can be consumed on another.
    parser = PyMftParser(str(SAMPLE))
    with concurrent.futures.ThreadPoolExecutor(max_workers=1) as pool:
        ids = pool.submit(
            lambda: [e.entry_id for e in itertools.islice(parser.entries(), 10)]
        ).result(timeout=30)
    assert ids[0] == 0
    assert len(ids) == 10


@pytest.mark.skipif(
    not sysconfig.get_config_var("Py_GIL_DISABLED"),
    reason="requires free-threaded CPython",
)
def test_import_and_parallel_parsers_keep_gil_disabled():
    # Use a fresh interpreter so earlier imports cannot hide an extension that
    # enables the GIL. A process timeout also bounds a potential deadlock.
    script = """
import concurrent.futures
import gc
import io
import itertools
from pathlib import Path
import sys
import threading

assert not sys._is_gil_enabled()
from mft import PyMftParser
assert not sys._is_gil_enabled(), "importing mft enabled the GIL"

path = sys.argv[1]
data = Path(path).read_bytes()
start = threading.Barrier(4)

def summarize(parser, method):
    out = []
    for entry in itertools.islice(getattr(parser, method)(), 500):
        if method == "entries":
            out.append((entry.entry_id, entry.full_path, len(list(entry.attributes()))))
        else:
            out.append(entry)
    return out

def parse(worker):
    start.wait(timeout=10)
    method = "entries_json" if worker % 2 else "entries"
    expected = summarize(PyMftParser(path), method)
    for _ in range(5):
        source = io.BytesIO(data) if worker < 2 else path
        assert summarize(PyMftParser(source), method) == expected
    return len(expected)

with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
    futures = [pool.submit(parse, worker) for worker in range(4)]
    for _ in range(10):
        gc.collect()
    assert [future.result(timeout=60) for future in futures] == [500] * 4
assert not sys._is_gil_enabled()
"""
    env = os.environ.copy()
    env.pop("PYTHON_GIL", None)
    result = subprocess.run(
        [sys.executable, "-c", script, str(SAMPLE)],
        env=env,
        capture_output=True,
        text=True,
        timeout=120,
    )
    assert result.returncode == 0, result.stdout + result.stderr
