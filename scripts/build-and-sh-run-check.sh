#!/usr/bin/env bash

set -euo pipefail

DATA_DIR="$(mktemp -d)"
trap 'rm -rf "$DATA_DIR"' EXIT

BASEDIR=$(dirname $0)
CMD="./target/debug/sql_rock --data-dir ${DATA_DIR}"


bash "$BASEDIR/select-join-query-tests.sh" "$CMD"

# テーブル作成して、selectのテスト
bash "$BASEDIR/select-query-tests1.sh" "$CMD"
# select+whereのテスト
bash "$BASEDIR/select-query-tests2.sh" "$CMD"
# select+集計関数のテスト
bash "$BASEDIR/select-query-tests3.sh" "$CMD"
# 複数データからの集計
bash "$BASEDIR/select-query-tests4.sh" "$CMD"
# select + is null
bash "$BASEDIR/select-query-tests5.sh" "$CMD"
# AUTO_INCREMENT
bash "$BASEDIR/select-query-tests6.sh" "$CMD"
# date & functions
bash "$BASEDIR/select-query-tests7.sh" "$CMD"
bash "$BASEDIR/update-delete-tests.sh" "$CMD"
bash "$BASEDIR/table-index-tests1.sh" "$CMD"
bash "$BASEDIR/create-table-basic-tests1.sh" "$CMD"

echo ok
