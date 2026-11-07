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

# Process spawning is confined to one module (architecture.md §5; Phase 3 §5).
echo "== checking std::process use =="
if grep -rn "std::process" crates/json/src crates/core/src crates/detectors/src \
  | grep -v "^crates/core/src/git.rs:"; then
  echo "FAIL: std::process used outside crates/core/src/git.rs"
  status=1
fi

if [ "$status" -eq 0 ]; then
  echo "OK: engine crates (json, core, detectors) have no network deps, only allowed internal edges, and process spawning only in core/src/git.rs"
fi

exit "$status"
