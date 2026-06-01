#!/usr/bin/env bash

set -euo pipefail

CMD=$1

echo "--- $0 start ---"

$CMD "CREATE TABLE users (id INT, name TEXT);"
$CMD "INSERT INTO users (id, name) VALUES (1, 'Alice');"
$CMD "INSERT INTO users (id, name) VALUES (2, 'Bob');"
$CMD "INSERT INTO users (id, name) VALUES (3, 'Noice');"

SQL_CASES=(
  "SELECT * FROM users ORDER BY id;"
  "SELECT count(id) FROM users;"
  "SELECT name FROM users WHERE id = 1;"
  "SELECT id,name FROM users WHERE id <> 1;"
  "SELECT id,name FROM users WHERE id < 3;"
  "SELECT * FROM users ORDER BY id desc limit 1;"
  "SELECT * FROM users ORDER BY id asc limit 1 offset 1;"
)

EXPECTED_CASES=(
  $'id\tname\n1\tAlice\n2\tBob\n3\tNoice'
  $'count(id)\n3'
  $'name\nAlice'
  $'id\tname\n2\tBob\n3\tNoice'
  $'id\tname\n1\tAlice\n2\tBob'
  $'id\tname\n3\tNoice'
  $'id\tname\n2\tBob'
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
