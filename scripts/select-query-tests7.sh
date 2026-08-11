#!/usr/bin/env bash

set -euo pipefail

CMD=$1
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

source "$SCRIPT_DIR/sql-test-helpers.sh"

echo "--- $0 start ---"

$CMD "CREATE TABLE items (\
   id INT AUTO_INCREMENT, \
   dt1 DATETIME, \
   dt2 DATE, \
   dt3 TIMESTAMP \
 );"
$CMD "INSERT INTO items (dt1, dt2, dt3) VALUES \
  (now(), CURDATE(), CURRENT_TIMESTAMP) \
;"

$CMD "SELECT * FROM items"
SQL_CASES=(
  "SELECT count(1) FROM items"
  "SELECT date_format(dt1, '%Y-%m-%d') as df FROM items"
  "SELECT date_format(dt2, '%Y-%m-%d') as df FROM items"
  "SELECT date_format(dt3, '%Y-%m-%d') as df FROM items"
)
#
EXPECTED_CASES=(
$'count(1)
1'
$'df
'$(date '+%Y-%m-%d')
$'df
'$(date '+%Y-%m-%d')
$'df
'$(date '+%Y-%m-%d')
)
# Success test patter
run_sql_cases "$CMD" SQL_CASES EXPECTED_CASES


$CMD "DROP TABLE items;"
echo "--- $0 finish ---"
