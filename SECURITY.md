# Security policy

## Supported versions

Until v0.1.0 is published, the private `master` branch is a Technical Preview
and receives security fixes. After publication, the latest `0.1.x` release is
supported; older preview commits and unverified NGINX build signatures are not.

## Reporting a vulnerability

Do not open a public issue for a suspected vulnerability. Use GitHub's private
security-advisory reporting for this repository and include:

- the affected commit or release and exact NGINX build signature;
- dynamic/static mode and vendored/system backend;
- minimal configuration and reproduction steps;
- expected and observed behavior;
- sanitizer output or a minimized input when available.

Do not include production secrets, private response data, or credentials.
Maintainers will acknowledge the report through the advisory thread, assess the
supported configuration, and coordinate disclosure after a fix is available.

## Security boundaries

The v0.1.0 contract excludes unverified NGINX versions/signatures, 0-RTT, and
compression dictionaries. A module built against a different NGINX ABI is not
a supported security configuration. Built-in `gzip on` conflicts fail closed;
disabling or bypassing that guard is unsupported.
