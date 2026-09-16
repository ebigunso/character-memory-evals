#!/usr/bin/env bash
set -euo pipefail
exec cargo run --manifest-path "$(dirname "$0")/../Cargo.toml" -p cmem-eval-locomo --example official_derived_content_census
