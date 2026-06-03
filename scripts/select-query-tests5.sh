#!/usr/bin/env bash

set -euo pipefail

CMD=$1
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

source "$SCRIPT_DIR/sql-test-helpers.sh"

echo "--- $0 start ---"

$CMD "CREATE TABLE items (\
   id INT, \
   name VARCHAR, \
   category TINYINT NOT NULL, \
   height INT, \
   weight INT \
 );"
$CMD "INSERT INTO items (id, name, category, height, weight) VALUES \
  (1, 'TV', 1, 10, 100), \
  (2, 'Chair', 1, NULL, 20), \
  (3, 'Light', 2, 5, NULL) \
;"
$CMD "INSERT INTO items (id, name, category) VALUES \
  (4, 'Light', 2) \
;"

SQL_CASES=(
  "SELECT count(1) FROM items"
  "SELECT count(1) FROM items where height is null"
  "SELECT count(1) FROM items where weight is not null"
)
#
EXPECTED_CASES=(
$'count(1)
4'
$'count(1)
2'
$'count(1)
2'
)
# Success test patter
run_sql_cases "$CMD" SQL_CASES EXPECTED_CASES


ERROR_SQL_CASES=(
  "INSERT INTO items (id, name, height, weight) VALUES (5, 'Light', 1, 1)"
)

ERROR_EXPECTED_CASES=(
  'error: column `category` cannot be NULL'
)

# Failure test patter
run_sql_error_cases "$CMD" ERROR_SQL_CASES ERROR_EXPECTED_CASES


$CMD "DROP TABLE items;"
echo "--- $0 finish ---"
