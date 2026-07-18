#!/usr/bin/env bash
# Guards the offline-engine invariant (architecture.md §5): json, core, and
# detectors must never gain a network dependency or an unlisted internal edge.
set -euo pipefail

# crate -> allowed internal (repo-intelligence-*) deps, space-separated
declare -A ALLOWED_INTERNAL=(
  [repo-intelligence-json]=""
  [repo-intelligence-core]="repo-intelligence-json"
  [repo-intelligence-detectors]="repo-intelligence-core repo-intelligence-json"
)

# crates/tools known to pull in network or async-runtime behavior
DENYLIST_REGEX='^(reqwest|hyper|tokio|ureq|curl|openssl-sys|native-tls|rustls|h2|tonic)( |$)'

status=0

for crate in "${!ALLOWED_INTERNAL[@]}"; do
  echo "== checking $crate =="
  tree=$(cargo tree -p "$crate" --edges normal --prefix none)

  while IFS= read -r line; do
    pkg=$(echo "$line" | awk '{print $1}')
    [ "$pkg" = "$crate" ] && continue

    if echo "$pkg" | grep -qE "$DENYLIST_REGEX"; then
      echo "FAIL: $crate pulls in denylisted network/async dependency: $pkg"
      status=1
    fi

    if [[ "$pkg" == repo-intelligence-* ]]; then
      allowed="${ALLOWED_INTERNAL[$crate]}"
      if [[ " $allowed " != *" $pkg "* ]]; then
        echo "FAIL: $crate has unexpected internal edge: $pkg"
        status=1
      fi
    fi
  done <<< "$tree"
done

if [ "$status" -eq 0 ]; then
  echo "OK: engine crates (json, core, detectors) have no network deps and only allowed internal edges"
fi

exit "$status"
