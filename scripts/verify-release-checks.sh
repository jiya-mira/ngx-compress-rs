#!/bin/sh
# Private-repository substitute for branch protection. Checks the exact commit.
set -eu

sha=${1:?target commit SHA is required}
repo=${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is required}
required='Rust / rust
NGINX integration / integration
HTTP/3 / http3
Security and release / security
Security and release / rehearsal'

checks=$(gh api --paginate "repos/$repo/commits/$sha/check-runs?per_page=100")
printf '%s\n' "$required" | while IFS= read -r name; do
    [ -n "$name" ] || continue
    conclusion=$(printf '%s' "$checks" | jq -r --arg name "$name" \
        '[.check_runs[] | select(.name == $name)] | sort_by(.completed_at) | last | .conclusion // "missing"')
    [ "$conclusion" = success ] \
        || { echo "required check '$name' is $conclusion for $sha"; exit 1; }
    echo "PASS $name"
done
