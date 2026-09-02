#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: qdrant_prune_collections.sh <collection-prefix> [--delete]" >&2
}

if (( $# < 1 || $# > 2 )) || { (( $# == 2 )) && [[ $2 != --delete ]]; }; then
  usage
  exit 2
fi

prefix=$1
if (( ${#prefix} < 12 )); then
  echo "error: collection prefix must contain at least 12 characters" >&2
  exit 2
fi

delete=false
[[ ${2-} == --delete ]] && delete=true
endpoint=${QDRANT_REST_URL:-http://127.0.0.1:6333}
endpoint=${endpoint%/}
response=$(curl -fsS "$endpoint/collections")
remaining=$response
name_pattern='"name"[[:space:]]*:[[:space:]]*"([^"]+)"'
matches=()

while [[ $remaining =~ $name_pattern ]]; do
  match=${BASH_REMATCH[0]}
  name=${BASH_REMATCH[1]}
  [[ $name == "$prefix"* ]] && matches+=("$name")
  remaining=${remaining#*"$match"}
done

for name in "${matches[@]}"; do
  if [[ $delete == true ]]; then
    curl -fsS -X DELETE "$endpoint/collections/$name" >/dev/null
    echo "deleted: $name"
  else
    echo "dry-run: $name"
  fi
done

echo "matched collections: ${#matches[@]}"
