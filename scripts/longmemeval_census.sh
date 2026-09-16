#!/usr/bin/env bash
set -euo pipefail
exec cargo run --manifest-path "$(dirname "$0")/../Cargo.toml" -p cmem-eval-longmemeval --example official_repeated_session_census
