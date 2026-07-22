## Summary

<!-- What changed, and why is this the smallest useful change? -->

## Behavioral and compatibility impact

<!-- Include configuration, ABI, filter-order, protocol, codec, and performance impact. -->

## Validation

<!-- List exact commands and results. Explain any check that was not run. -->

## Checklist

- [ ] The change is focused and preserves documented compatibility, or the break is explicit.
- [ ] Tests cover the main path and a relevant failure or boundary path.
- [ ] New or changed `unsafe` is confined to the FFI boundary and documents its invariants.
- [ ] No panic can cross an NGINX callback boundary.
- [ ] User-facing configuration or support changes are documented.
- [ ] No credentials, private data, production configuration, or unsanitized logs are included.
- [ ] I have read `CONTRIBUTING.md` and will follow `CODE_OF_CONDUCT.md`.
