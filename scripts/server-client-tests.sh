#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
DATA_DIR="$(mktemp -d)"
SERVER_LOG="$(mktemp)"
PORT="${SQL_ROCK_SERVER_TEST_PORT:-13307}"
SERVER_PID=""

cleanup() {
  if [[ -n "$SERVER_PID" ]]; then
    kill "$SERVER_PID" 2>/dev/null || true
    wait "$SERVER_PID" 2>/dev/null || true
  fi
  rm -rf "$DATA_DIR"
  rm -f "$SERVER_LOG"
}
trap cleanup EXIT

"$ROOT_DIR/target/debug/sql_rock_server" \
  --host 127.0.0.1 \
  --port "$PORT" \
  --data-dir "$DATA_DIR" >"$SERVER_LOG" 2>&1 &
SERVER_PID=$!

for _ in {1..50}; do
  if MYSQL_PWD=sqlrock "$ROOT_DIR/target/debug/sql_rock_client" \
    --host 127.0.0.1 \
    --port "$PORT" \
    --user sqlrock \
    --execute "SHOW TABLES" >/dev/null 2>&1; then
    break
  fi
  if ! kill -0 "$SERVER_PID" 2>/dev/null; then
    cat "$SERVER_LOG"
    exit 1
  fi
  sleep 0.1
done

OUTPUT="$(
  printf '%s\n' \
    "CREATE TABLE users (id INT PRIMARY KEY, name TEXT);" \
    "INSERT INTO users (id, name) VALUES (123, 'Alice');" \
    "PREPARE stmt FROM 'SELECT * FROM users WHERE id = ?';" \
    "EXECUTE stmt USING 123;" \
    "DEALLOCATE PREPARE stmt;" |
    MYSQL_PWD=sqlrock "$ROOT_DIR/target/debug/sql_rock_client" \
      --host 127.0.0.1 \
      --port "$PORT" \
      --user sqlrock
)"

EXPECTED=$'Query OK, 0 row(s) affected\nQuery OK, 1 row(s) affected\nQuery OK, 0 row(s) affected\nid\tname\n123\tAlice\nQuery OK, 0 row(s) affected'

if [[ "$OUTPUT" != "$EXPECTED" ]]; then
  printf 'Unexpected server/client output.\nExpected:\n%s\nActual:\n%s\n' "$EXPECTED" "$OUTPUT"
  exit 1
fi

NOW_OUTPUT="$(
  MYSQL_PWD=sqlrock "$ROOT_DIR/target/debug/sql_rock_client" \
    --host 127.0.0.1 \
    --port "$PORT" \
    --user sqlrock \
    --execute "SELECT NOW()"
)"

if [[ ! "$NOW_OUTPUT" =~ ^now\(\)$'\n'[0-9]{4}-[0-9]{2}-[0-9]{2}\ [0-9]{2}:[0-9]{2}:[0-9]{2}$ ]]; then
  printf 'Unexpected SELECT NOW() output:\n%s\n' "$NOW_OUTPUT"
  exit 1
fi

if MYSQL_PWD=wrong "$ROOT_DIR/target/debug/sql_rock_client" \
  --host 127.0.0.1 \
  --port "$PORT" \
  --user sqlrock \
  --execute "SHOW TABLES" >/dev/null 2>&1; then
  echo "Authentication unexpectedly succeeded with an invalid password."
  exit 1
fi

echo "sql_rock_server/client integration checks passed"
