# Release, tag, and rollback runbook

This runbook is for the source-only v0.1.0 Technical Preview. Public visibility,
tagging, and GitHub Release creation require explicit final authorization.

## 1. Freeze the candidate

1. Confirm NGINX 1.30.4 is still the latest 1.30 stable patch. If not, update
   every pin and checksum and rerun all checks.
2. Require a clean worktree and an exact private remote commit:

```sh
git status --short
git rev-parse HEAD
git rev-parse origin/master
```

1. Run the private exact-commit gate:

```sh
gh workflow run security-release.yml -f target_sha="$(git rev-parse HEAD)"
```

## 2. Build release evidence

```sh
scripts/package-release.sh
unzip -t dist/ngx-compress-rs-0.1.0-source.zip
cat dist/ngx-compress-rs-0.1.0-source.zip.sha256
```

Download the successful `supply-chain-<sha>` and HTTP/3 benchmark artifacts.
The release bundle must contain:

- `ngx-compress-rs-0.1.0-source.zip`;
- its `.sha256` and `.contents.txt` files;
- vendored and system CycloneDX JSON SBOMs;
- toolchain manifest;
- committed HTTP/3 benchmark evidence.

## 3. Fresh-checkout rehearsal

Clone into a new directory. Follow [installation.md](installation.md) once with
`--add-dynamic-module` and once with `--add-module`, using the supported exact
signature. For both, run `nginx -t`, a graceful reload, and byte-for-byte
H1/H2/H3 decoding checks. HTTP/3 clients must use `--http3-only`.

## 4. Publish after authorization

After the user explicitly authorizes the final gate:

1. change repository visibility to public;
2. enable required checks on `master`;
3. create and push the annotated tag:

```sh
git tag -a v0.1.0 -m 'ngx-compress-rs v0.1.0 Technical Preview'
git push origin v0.1.0
```

1. create the GitHub Release from [releases/v0.1.0.md](releases/v0.1.0.md) and
   attach all release evidence;
2. repeat the public source download and installation smoke.

## Rollback

- Do not delete or move the tag. Mark a broken release as withdrawn and publish
  a corrective patch release.
- Dynamic deployment: restore the previous `.so` built for the same exact NGINX
  signature, run `nginx -t`, then gracefully reload.
- Static deployment: restore the previous NGINX executable and matching config,
  run its configuration test, then switch/reload using the deployment's atomic
  process.
- If configuration alone caused the failure, set `compress off`, validate, and
  reload. Keep built-in gzip ownership explicit; do not bypass fail-closed
  conflict handling.
- Preserve failed logs, checksums, SBOMs, and the exact commit for incident
  analysis.
