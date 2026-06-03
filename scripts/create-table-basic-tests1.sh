#!/usr/bin/env bash

set -euo pipefail

CMD=$1
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

source "$SCRIPT_DIR/sql-test-helpers.sh"

echo "--- $0 start ---"

$CMD "CREATE TABLE items (\
   id INT AUTO_INCREMENT, \
   name VARCHAR \
 ) comment='コメント';"
$CMD "CREATE TABLE items2 (\
   id INT AUTO_INCREMENT, \
   name VARCHAR \
 ) AUTO_INCREMENT=123001;"
$CMD "INSERT INTO items (name) VALUES \
  ('TV'), \
  ('Chair') \
;"
$CMD "INSERT INTO items2 (name) VALUES \
  ('TV'), \
  ('Chair') \
;"

SQL_CASES=(
  "SELECT count(1) FROM items"
  "SELECT * FROM items2 ORDER BY id;"
)
#
EXPECTED_CASES=(
$'count(1)
2'
$'id	name
123001	TV
123002	Chair'
)

run_sql_cases "$CMD" SQL_CASES EXPECTED_CASES


$CMD "DROP TABLE items;"
echo "--- $0 finish ---"
