# v0.1.0 HTTP/3 balanced-buffer evidence

This directory preserves the release benchmark produced by GitHub Actions run
[`29922060488`](https://github.com/jiya-mira/ngx-compress-rs/actions/runs/29922060488)
from source commit `d1093cf74314543b13951c741f798ff261dc7100`.

The matrix compares the unified 4, 8, 16, and 32 KiB output-buffer settings
across HTTP/1.1, HTTP/2, and HTTP/3. It covers 4 KiB, 256 KiB, and 8 MiB
compressible and seeded-incompressible payloads, with one warm-up followed by
five measured rounds for every cell. `http3-raw.tsv` contains 360 measurements;
`toolchain.txt` records the runner and pinned protocol stack; `conclusion.md`
contains the deterministic result from `docker/http3/analyze.py`.

No candidate met the release gate, so v0.1.0 retains the 8 KiB unified default
and adds no protocol-specific buffer or flush behavior.
