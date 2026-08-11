#!/usr/bin/env bash

set -euo pipefail

CMD=$1
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

source "$SCRIPT_DIR/sql-test-helpers.sh"

echo "--- $0 start ---"

$CMD "CREATE TABLE items (\
   id INT AUTO_INCREMENT, \
   name DATETIME \
 );
 CREATE TABLE item_details (\
   item_id INT, \
   stock int \
 );"
$CMD "INSERT INTO items (id, name) VALUES \
  (100, 'item001'), \
  (101, 'item002'), \
  (102, 'item003') \
;"
$CMD "INSERT INTO item_details (item_id, stock) VALUES \
  (100, 1), \
  (101, 2) \
;"

SQL_CASES=(
  "SELECT count(1) FROM items"
  "SELECT id, name, stock FROM items i inner join item_details ii on i.id = ii.item_id"
  "SELECT id, name, stock FROM items i left join item_details ii on i.id = ii.item_id"
  "SELECT id, name, stock FROM items i inner join item_details ii on i.id = ii.item_id where ii.stock = 2"
  "SELECT id, name, stock FROM items i left join item_details ii on i.id = ii.item_id where ii.stock is not null"
)
#
EXPECTED_CASES=(
$'count(1)
3'
$'id	name	stock
100	item001	1
101	item002	2'
$'id	name	stock
100	item001	1
101	item002	2
102	item003	'
$'id	name	stock
101	item002	2'
$'id	name	stock
100	item001	1
101	item002	2'
)
# Success test patter
run_sql_cases "$CMD" SQL_CASES EXPECTED_CASES


$CMD "DROP TABLE items;"
$CMD "DROP TABLE item_details;"
echo "--- $0 finish ---"
