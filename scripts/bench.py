"""Rough throughput benchmark for the Python bindings.

usage: python scripts/bench.py [path/to/MFT] [--repeat N]
"""
import argparse
import statistics
import time
from pathlib import Path

from mft import PyMftParser

DEFAULT_SAMPLE = Path(__file__).resolve().parent.parent / "samples" / "MFT"


def entries_only(path):
    n = 0
    for entry in PyMftParser(path).entries():
        n += 1
    return n


def entries_with_timestamps(path):
    """Typical timeline use: walk every attribute and read its timestamps."""
    n = 0
    for entry in PyMftParser(path).entries():
        for attribute in entry.attributes():
            content = attribute.attribute_content
            created = getattr(content, "created", None)
            if created is not None:
                content.modified, content.mft_modified, content.accessed
                n += 4
    return n


def entries_json(path):
    return sum(len(s) for s in PyMftParser(path).entries_json())


def entries_csv(path):
    return sum(len(b) for b in PyMftParser(path).entries_csv())


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("path", nargs="?", default=str(DEFAULT_SAMPLE))
    parser.add_argument("--repeat", type=int, default=7)
    args = parser.parse_args()

    for bench in (entries_only, entries_with_timestamps, entries_json, entries_csv):
        bench(args.path)  # warmup
        times = []
        for _ in range(args.repeat):
            start = time.perf_counter()
            bench(args.path)
            times.append(time.perf_counter() - start)
        print(
            f"{bench.__name__:24} median {statistics.median(times) * 1e3:8.1f} ms"
            f"  min {min(times) * 1e3:8.1f} ms"
        )


if __name__ == "__main__":
    main()
