#!/usr/bin/env bash

set -euo pipefail

DATA_DIR="$(mktemp -d)"
trap 'rm -rf "$DATA_DIR"' EXIT

BASEDIR=$(dirname $0)
CMD="./target/debug/sql_rock --data-dir ${DATA_DIR}"

bash "$BASEDIR/select-query-tests1.sh" "$CMD"
bash "$BASEDIR/select-query-tests2.sh" "$CMD"
bash "$BASEDIR/select-query-tests3.sh" "$CMD"
bash "$BASEDIR/select-query-tests4.sh" "$CMD"
bash "$BASEDIR/select-query-tests5.sh" "$CMD"
bash "$BASEDIR/select-query-tests6.sh" "$CMD"
bash "$BASEDIR/update-delete-tests.sh" "$CMD"
bash "$BASEDIR/table-index-tests1.sh" "$CMD"
bash "$BASEDIR/create-table-basic-tests1.sh" "$CMD"

echo ok
