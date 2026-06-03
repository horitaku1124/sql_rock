#!/usr/bin/env bash

set -euo pipefail

CMD=$1
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

source "$SCRIPT_DIR/sql-test-helpers.sh"

echo "--- $0 start ---"

$CMD "CREATE TABLE items (\
   id INT AUTO_INCREMENT, \
   name VARCHAR \
 );"
$CMD "INSERT INTO items (name) VALUES \
  ('TV'), \
  ('Chair') \
;"
$CMD "INSERT INTO items (id,name) VALUES \
  (3, 'Light') \
;"

SQL_CASES=(
  "SELECT count(1) FROM items"
  "SELECT * FROM items ORDER BY id"
)
#
EXPECTED_CASES=(
$'count(1)
3'
$'id	name
1	TV
2	Chair
3	Light'

)
# Success test patter
run_sql_cases "$CMD" SQL_CASES EXPECTED_CASES

# TODO should be error
#ERROR_SQL_CASES=(
#  "INSERT INTO items (id, name) VALUES (1, 'Light2')"
#)
#
#ERROR_EXPECTED_CASES=(
#  'error: column `category` cannot be NULL'
#)
#
## Failure test patter
#run_sql_error_cases "$CMD" ERROR_SQL_CASES ERROR_EXPECTED_CASES


$CMD "DROP TABLE items;"
echo "--- $0 finish ---"
