#!/usr/bin/env bash

set -euo pipefail

CMD=$1

echo "--- $0 start ---"

$CMD "CREATE TABLE users (id INT, name TEXT);"
$CMD "INSERT INTO users (id, name) VALUES (1, 'Alice');"

SQL_CASES=(
  "SELECT * FROM users;"
  "SELECT count(id) FROM users;"
  "SELECT name FROM users WHERE id = 1;"
  "SELECT id FROM users WHERE name = 'Alice';"
)

EXPECTED_CASES=(
  $'id\tname\n1\tAlice'
  $'count(id)\n1'
  $'name\nAlice'
  $'id\n1'
)

for i in "${!SQL_CASES[@]}"; do
  OUTPUT="$($CMD "${SQL_CASES[$i]}")"
  EXPECTED="${EXPECTED_CASES[$i]}"

  if [[ "$OUTPUT" != "$EXPECTED" ]]; then
    printf 'failed: %s\nexpected:\n%s\nactual:\n%s\n' \
      "${SQL_CASES[$i]}" "$EXPECTED" "$OUTPUT"
    exit 1
  fi
done

$CMD "DROP TABLE users;"
echo "--- $0 finish ---"
