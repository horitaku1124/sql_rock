#!/usr/bin/env bash

set -euo pipefail

CMD=$1
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

source "$SCRIPT_DIR/sql-test-helpers.sh"

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

run_sql_cases "$CMD" SQL_CASES EXPECTED_CASES

$CMD "DROP TABLE users;"
echo "--- $0 finish ---"
