#!/bin/sh

set -eu

DATA_DIR="$(mktemp -d)"

echo $DATA_DIR

./target/debug/sql_rock --data-dir "$DATA_DIR" "CREATE TABLE users (id INT, name TEXT);"
./target/debug/sql_rock --data-dir "$DATA_DIR" "INSERT INTO users (id, name) VALUES (1, 'Alice');"
OUTPUT="$(./target/debug/sql_rock --data-dir "$DATA_DIR" \
  "SELECT * FROM users;")"


EXPECTED="$(printf 'id\tname\n1\tAlice')"


set +eu
rm "$DATA_DIR/"*.table

if [ "$OUTPUT" != "$EXPECTED" ]; then
  printf 'unexpected output:\n%s\n' "$OUTPUT"
  exit 1
fi

echo ok