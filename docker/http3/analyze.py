#!/usr/bin/env python3
"""Apply the v0.1.0 unified-buffer acceptance gate to raw benchmark rows."""

from __future__ import annotations

import csv
import statistics
import sys
from collections import defaultdict
from pathlib import Path


def median(values: list[float]) -> float:
    return statistics.median(values)


def main(raw_path: Path, output_path: Path) -> None:
    samples: dict[tuple[int, str, str], dict[str, list[float]]] = defaultdict(
        lambda: defaultdict(list)
    )
    with raw_path.open(newline="", encoding="utf-8") as raw:
        for row in csv.DictReader(raw, delimiter="\t"):
            key = (int(row["buffer_kib"]), row["protocol"], row["payload"])
            for metric in ("ttfb_s", "total_s", "speed_Bps", "compressed_bytes", "worker_rss_kib"):
                samples[key][metric].append(float(row[metric]))

    summary = {
        key: {metric: median(values) for metric, values in metrics.items()}
        for key, metrics in samples.items()
    }
    candidates: list[tuple[int, float, bool, list[str]]] = []
    payloads = sorted({key[2] for key in summary})
    for buffer in (4, 16, 32):
        h3_gains: list[float] = []
        violations: list[str] = []
        for payload in payloads:
            for protocol in ("h1", "h2", "h3"):
                base = summary[(8, protocol, payload)]
                candidate = summary[(buffer, protocol, payload)]
                speed_delta = candidate["speed_Bps"] / base["speed_Bps"] - 1
                ttfb_delta = candidate["ttfb_s"] / base["ttfb_s"] - 1
                rss_delta = candidate["worker_rss_kib"] / base["worker_rss_kib"] - 1
                if protocol == "h3":
                    h3_gains.append(speed_delta)
                if protocol in ("h1", "h2") and speed_delta < -0.05:
                    violations.append(f"{protocol}/{payload} throughput {speed_delta:.1%}")
                if ttfb_delta > 0.10:
                    violations.append(f"{protocol}/{payload} TTFB +{ttfb_delta:.1%}")
                if rss_delta > 0.10:
                    violations.append(f"{protocol}/{payload} RSS +{rss_delta:.1%}")
        h3_gain = median(h3_gains)
        accepted = h3_gain >= 0.10 and not violations
        candidates.append((buffer, h3_gain, accepted, violations))

    accepted = [item for item in candidates if item[2]]
    winner = max(accepted, key=lambda item: item[1], default=None)
    decision = f"replace 8 KiB with {winner[0]} KiB" if winner else "retain the 8 KiB default"
    lines = [
        "# HTTP/3 balanced-buffer conclusion",
        "",
        f"Decision: **{decision}**.",
        "",
        "The gate requires at least 10% median HTTP/3 throughput improvement, no more than 5% H1/H2 throughput regression, 10% TTFB regression, or 10% worker RSS regression for every workload.",
        "",
        "## Candidates",
        "",
    ]
    lines.extend(
        f"- {buffer} KiB: median HTTP/3 throughput {gain:+.1%}; "
        f"gate {'pass' if accepted else 'fail'}"
        for buffer, gain, accepted, _ in candidates
    )
    lines.extend(["", "## Violations", ""])
    for buffer, _, _, violations in candidates:
        if violations:
            lines.append(f"- {buffer} KiB: " + "; ".join(violations))
        else:
            lines.append(f"- {buffer} KiB: none")
    output_path.write_text("\n".join(lines) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main(Path(sys.argv[1]), Path(sys.argv[2]))
