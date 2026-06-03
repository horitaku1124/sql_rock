#!/usr/bin/env bash

set -euo pipefail

CMD=$1
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

source "$SCRIPT_DIR/sql-test-helpers.sh"

echo "--- $0 start ---"

$CMD "CREATE TABLE items (\
   id INT, \
   code CHAR NOT NULL,\
  PRIMARY KEY (code)
 );"
$CMD "CREATE TABLE items2 (\
   id INT, \
   code CHAR NOT NULL,\
  UNIQUE KEY (code)
 );"
$CMD "INSERT INTO items (id, code) VALUES \
  (1,'tv'), \
  (2,'pc') \
;"
$CMD "INSERT INTO items2 (id, code) VALUES \
  (1,'tv'), \
  (2,'pc') \
;"
SQL_CASES=(
  "SELECT count(1) FROM items"
  "SELECT * FROM items ORDER BY id"
  "SELECT count(1) FROM items2"
  "SELECT * FROM items2 ORDER BY id"
)
#
EXPECTED_CASES=(
$'count(1)
2'
$'id	code
1	tv
2	pc'
$'count(1)
2'
$'id	code
1	tv
2	pc'

)
# Success test patter
run_sql_cases "$CMD" SQL_CASES EXPECTED_CASES


ERROR_SQL_CASES=(
  "INSERT INTO items (id,code) VALUES (3, 'tv');"
  "INSERT INTO items2 (id,code) VALUES (3, 'tv');"
)

ERROR_EXPECTED_CASES=(
  'error: duplicate value `tv` for key `code`'
  'error: duplicate value `tv` for key `code`'
)

# Failure test patter
run_sql_error_cases "$CMD" ERROR_SQL_CASES ERROR_EXPECTED_CASES


$CMD "DROP TABLE items;"
echo "--- $0 finish ---"
