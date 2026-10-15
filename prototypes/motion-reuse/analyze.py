#!/usr/bin/env python3
"""R4: tables from motion-reuse runs (frames.csv per flight).

usage: analyze.py <out-dir> [flight ...]
Per flight and strategy: iterations per gesture frame (absolute, vs FL's model, vs full
resolution), new samples per frame, ΔE mean / p99 against the converged reference, temporal
error; per zoom phase for long dives; and iso-quality cost: the cheapest cache variant whose
mean ΔE and temporal error are both no worse than FL's (interpolated along sc-b*).
"""
import collections
import csv
import math
import os
import sys


def load(path):
    rows = list(csv.DictReader(open(path)))
    return rows


def stats(rows, keyf=lambda r: 0):
    out = collections.OrderedDict()
    for r in rows:
        key = (keyf(r), r["strategy"])
        a = out.setdefault(key, dict(n=0, cost=0.0, new=0.0, ev=0, de=0.0, p99=0.0, tf=0.0))
        a["n"] += 1
        a["cost"] += float(r["cost"])
        a["new"] += float(r["new_samples"])
        if r["de_mean"]:
            a["ev"] += 1
            a["de"] += float(r["de_mean"])
            a["p99"] += float(r["de_p99"])
            a["tf"] += float(r["flicker"])
    return out


def table(st, phase):
    strategies = [s for (p, s) in st if p == phase]
    fl = st[(phase, "fl")]
    full = st[(phase, "fl-full")]
    flc = fl["cost"] / fl["n"]
    fullc = full["cost"] / full["n"]
    lines = []
    lines.append("| strategy | iter/frame | vs FL | vs full | new/frame | ΔE mean | ΔE p99 | temporal err |")
    lines.append("|---|---:|---:|---:|---:|---:|---:|---:|")
    for s in strategies:
        a = st[(phase, s)]
        if s == "reference":
            continue
        c = a["cost"] / a["n"]
        ev = max(a["ev"], 1)
        lines.append(
            f"| {s} | {c:.3g} | {c / flc:.2f} | {c / fullc:.3f} | {a['new'] / a['n']:.0f} | "
            f"{a['de'] / ev:.2f} | {a['p99'] / ev:.1f} | {a['tf'] / ev:.2f} |"
        )
    return "\n".join(lines)


def main():
    out = sys.argv[1]
    flights = sys.argv[2:] or sorted(d for d in os.listdir(out) if os.path.isfile(os.path.join(out, d, "frames.csv")))
    for f in flights:
        rows = load(os.path.join(out, f, "frames.csv"))
        n = len({r["k"] for r in rows})
        print(f"\n### {f} ({n} gesture frames)\n")
        st = stats(rows)
        print(table(st, 0))
        zooms = [float(r["zoom"]) for r in rows]
        if max(zooms) / max(min(zooms), 1e-300) > 1e4:
            def phase(r):
                return int(math.floor(math.log10(max(float(r["zoom"]), 1e-300)) / 3))
            stp = stats(rows, phase)
            for p in sorted({p for (p, _) in stp}):
                print(f"\nzoom 1e{3 * p} .. 1e{3 * p + 3}:\n")
                print(table(stp, p))


if __name__ == "__main__":
    main()
