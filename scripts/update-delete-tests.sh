#!/usr/bin/env bash

set -euo pipefail

CMD=$1
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

source "$SCRIPT_DIR/sql-test-helpers.sh"

echo "--- $0 start ---"

$CMD "CREATE TABLE users (\
   id INT, \
   name TEXT NOT NULL, \
   status TEXT, \
   age INT \
 );"
$CMD "INSERT INTO users (id, name, status, age) VALUES \
  (1, 'Alice', 'ACTIVE', 20), \
  (2, 'Bob', 'ACTIVE', 25), \
  (3, 'Carol', 'PENDING', 30) \
;"

SQL_CASES=(
  "UPDATE users SET status = 'INACTIVE' WHERE id = 2"
  "SELECT * FROM users WHERE id = 2"
  "UPDATE users SET name = 'Caroline', status = 'ACTIVE' WHERE id = 3"
  "SELECT id, name, status FROM users ORDER BY id"
  "UPDATE users SET status = 'MISSING' WHERE id = 999"
  "DELETE FROM users WHERE id = 1"
  "SELECT count(id) FROM users"
  "DELETE FROM users WHERE id = 999"
  "SELECT * FROM users ORDER BY id"
)

EXPECTED_CASES=(
  "updated 1 row(s) in \`users\`"
  $'id\tname\tstatus\tage\n2\tBob\tINACTIVE\t25'
  "updated 1 row(s) in \`users\`"
  $'id\tname\tstatus\n1\tAlice\tACTIVE\n2\tBob\tINACTIVE\n3\tCaroline\tACTIVE'
  "updated 0 row(s) in \`users\`"
  "deleted 1 row(s) from \`users\`"
  $'count(id)\n2'
  "deleted 0 row(s) from \`users\`"
  $'id\tname\tstatus\tage\n2\tBob\tINACTIVE\t25\n3\tCaroline\tACTIVE\t30'
)

run_sql_cases "$CMD" SQL_CASES EXPECTED_CASES

ERROR_SQL_CASES=(
  "UPDATE users SET missing = 'x' WHERE id = 2"
  "UPDATE users SET name = NULL WHERE id = 2"
  "DELETE FROM users WHERE missing = 1"
)

ERROR_EXPECTED_CASES=(
  'error: unknown column `missing` for table `users`'
  'error: column `name` cannot be NULL'
  'error: unknown column `missing` for table `users`'
)

run_sql_error_cases "$CMD" ERROR_SQL_CASES ERROR_EXPECTED_CASES

POST_ERROR_SQL_CASES=(
  "DELETE FROM users"
  "SELECT * FROM users"
)

POST_ERROR_EXPECTED_CASES=(
  "deleted 2 row(s) from \`users\`"
  $'id\tname\tstatus\tage'
)

run_sql_cases "$CMD" POST_ERROR_SQL_CASES POST_ERROR_EXPECTED_CASES

$CMD "DROP TABLE users;"
echo "--- $0 finish ---"
