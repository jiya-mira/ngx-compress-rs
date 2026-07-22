# HTTP/3 balanced-buffer conclusion

Decision: **retain the 8 KiB default**.

The gate requires at least 10% median HTTP/3 throughput improvement, no more than 5% H1/H2 throughput regression, 10% TTFB regression, or 10% worker RSS regression for every workload.

## Candidates

- 4 KiB: median HTTP/3 throughput +1.8%; gate fail
- 16 KiB: median HTTP/3 throughput -1.3%; gate fail
- 32 KiB: median HTTP/3 throughput -0.7%; gate fail

## Violations

- 4 KiB: h1/incompressible-4k.bin throughput -10.7%; h1/incompressible-4k.bin TTFB +11.9%
- 16 KiB: h1/incompressible-4k.bin throughput -7.2%
- 32 KiB: h1/incompressible-4k.bin throughput -7.1%
