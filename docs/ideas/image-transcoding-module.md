# Idea: image transcoding / optimization (sibling module)

**Status: exploratory. NOT part of `ngx-compress-rs`.** This is a parked idea for
a *separate* module and repository. It lives here only as a seed for future
evaluation; once that repo is started, delete this file and migrate its content
there. Nothing in `ngx-compress-rs` depends on it.

`ngx-compress-rs` is an HTTP content-coding compressor (`Content-Encoding`:
gzip / deflate / brotli / zstd, and later dcb / dcz). Image transcoding is a
different problem and is deliberately out of its scope — see
[Why it is separate](#why-it-is-separate-from-the-content-coding-module).

## The idea

Serve modern image formats (WebP, AVIF, possibly JPEG XL) by transcoding source
images — on demand or ahead of time — negotiating the best format the client
accepts and caching the result.

## Why it is separate from the content-coding module

- **Different HTTP mechanism.** Content coding is lossless, transparent, and
  negotiated with `Accept-Encoding` → `Content-Encoding`; the client decodes back
  to identical bytes. Image transcoding is (usually) lossy, changes the resource
  itself, negotiates with `Accept` / client hints → `Content-Type`, and needs
  `Vary: Accept`. The resource identity changes.
- **Inverted performance profile.** This module bounds per-invocation work to
  stay on the NGINX event loop; AVIF / JXL encode is seconds-scale CPU that must
  run off-loop and be cached, or the same image is re-encoded every request.
- **Needs a variant cache** (source × format × quality × dimensions) — which is
  an explicit non-goal of the content-coding module.
- **Dependency and safety surface.** libvips / libaom / dav1d / mozjpeg /
  libwebp / ICC / EXIF — image decoders are a classic RCE surface and would
  dilute the narrow FFI boundary that is this module's core value.
- **Different negotiation engine.** `Accept` + client hints + `Vary: Accept`
  (+ cache-key normalization) shares almost nothing with `Accept-Encoding`
  q-value negotiation.

Only architectural *patterns* carry over as shared lineage — the FFI discipline
(`FFI prefetch → safe Rust core → typed submit → FFI`) and the bounded/async work
contract — not shared code in the same crate.

## Landscape / maturity snapshot (early 2026)

Primitives are mature; the product space is crowded and largely solved — so a new
entry needs a differentiation thesis, not just existence.

- **Codec primitives (mature; never reimplement).** AVIF via `rav1e` / `ravif`
  (pure Rust, within single-digit-% of libaom / SVT-AV1, which lead at
  quality-first); WebP via libwebp; JPEG XL **decode** now via the Rust `jxl-rs`
  (merged into Chromium in January 2026), but JXL **encode** still leans on the
  C `libjxl`.
- **Dedicated proxies / SaaS / edge (very mature).** imgproxy (Go + libvips),
  Thumbor, Cloudinary, imgix, Cloudflare Images, Fastly IO — on-the-fly resize +
  format negotiation + caching, at the cost of a separate service / extra hop.
- **In-NGINX (no longer a clean gap).** The built-in `image_filter` is a libgd
  resizer with no AVIF or smart negotiation. `mod_pagespeed` / `ngx_pagespeed`
  were retired by Google (2023) but revived and maintained by We-Amp as
  **ModPageSpeed**. `ngx_immerse` is a native module that auto-transcodes to
  WebP / AVIF, negotiates on `Accept`, and caches variants.

## Negotiation notes

- Format negotiation is generic HTTP proactive negotiation: browsers advertise
  `image/avif,image/webp,…` in `Accept`; the server picks and returns
  `Vary: Accept`. There is no image-specific protocol.
- Practical pain: `Vary: Accept` fragments caches (CDNs normalize `Accept` to a
  few buckets — supports-avif / supports-webp / neither); Safari has historically
  under-advertised support, pushing some servers back to User-Agent sniffing.
- Client Hints (`Sec-CH-DPR`, `Sec-CH-Width`, `Save-Data`) cover
  resolution / data-saver, not format.
- The `<picture>` + `<source type="image/avif">` markup approach sidesteps server
  negotiation and the `Vary` cache problem entirely — often the recommended path.

## The only defensible wedge (if pursued)

- **Memory safety.** A Rust-first, safety-boundaried image pipeline rides the same
  wave as Chromium switching JXL decode to `jxl-rs`. Audience: security-conscious
  operators — incumbents are C / Go and "good enough" for most.
- **Perceptual-quality targeting.** SSIMULACRA2 / butteraugli-guided per-image
  adaptive quality — hard, and where even mature tools differ.
- **Architecture.** It wants to be a standalone service / sidecar (its own cache,
  async encode), not an NGINX-embedded module.

## Sources

- [Serving WebP & AVIF with NGINX (Vincent Bernat)](https://vincent.bernat.ch/en/blog/2021-webp-avif-nginx)
- [ModPageSpeed — maintained continuation](https://modpagespeed.com/)
- [ravif — Rust AVIF encoder (lib.rs)](https://lib.rs/crates/ravif)
- [Google restores JPEG XL to Chromium via a Rust decoder (TechRadar)](https://www.techradar.com/pro/google-restores-much-missed-jpeg-xl-format-to-chromium-code-base-better-image-compression-and-better-bandwidth-are-on-the-way)
- [Optimize images / serve WebP with NGINX + imgproxy (Sail blog)](https://blog.sailed.io/imgproxy-wordpress-optimize-images-webp/)
