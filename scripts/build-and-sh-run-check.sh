#!/usr/bin/env bash

set -euo pipefail

DATA_DIR="$(mktemp -d)"
trap 'rm -rf "$DATA_DIR"' EXIT

./target/debug/sql_rock --data-dir "$DATA_DIR" "CREATE TABLE users (id INT, name TEXT);"
./target/debug/sql_rock --data-dir "$DATA_DIR" "INSERT INTO users (id, name) VALUES (1, 'Alice');"

SQL_CASES=(
  "SELECT * FROM users;"
  "SELECT count(id) FROM users;"
  "SELECT * FROM users WHERE id = 1;"
)

EXPECTED_CASES=(
  $'id\tname\n1\tAlice'
  $'count(id)\n1'
  $'id\tname\n1\tAlice'
)

for i in "${!SQL_CASES[@]}"; do
  OUTPUT="$(./target/debug/sql_rock --data-dir "$DATA_DIR" "${SQL_CASES[$i]}")"
  EXPECTED="${EXPECTED_CASES[$i]}"

  if [[ "$OUTPUT" != "$EXPECTED" ]]; then
    printf 'failed: %s\nexpected:\n%s\nactual:\n%s\n' \
      "${SQL_CASES[$i]}" "$EXPECTED" "$OUTPUT"
    exit 1
  fi
done

echo ok
