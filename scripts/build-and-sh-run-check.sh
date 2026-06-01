#!/usr/bin/env bash

set -euo pipefail

DATA_DIR="$(mktemp -d)"
trap 'rm -rf "$DATA_DIR"' EXIT

BASEDIR=$(dirname $0)
CMD="./target/debug/sql_rock --data-dir ${DATA_DIR}"

bash "$BASEDIR/select-query-tests1.sh" "$CMD"
bash "$BASEDIR/select-query-tests2.sh" "$CMD"

echo ok
