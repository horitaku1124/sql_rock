#!/usr/bin/env bash

set -euo pipefail

CMD=$1
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

source "$SCRIPT_DIR/sql-test-helpers.sh"

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

run_sql_cases "$CMD" SQL_CASES EXPECTED_CASES

$CMD "DROP TABLE users;"
echo "--- $0 finish ---"
