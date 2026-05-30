#!/bin/sh

set -eu

DATA_DIR="$(mktemp -d)"
OUTPUT="$(./target/debug/sql_rock --data-dir "$DATA_DIR" \
  "CREATE TABLE users (id INT, name TEXT);
   INSERT INTO users (id, name) VALUES (1, 'Alice');
   SELECT * FROM users;")"

EXPECTED="$(printf 'created table `%s`\ninserted 1 row into `%s`\nid\tname\n1\tAlice' users users)"

if [ "$OUTPUT" != "$EXPECTED" ]; then
  printf 'unexpected output:\n%s\n' "$OUTPUT"
  exit 1
fi
