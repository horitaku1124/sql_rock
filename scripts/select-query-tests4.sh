#!/usr/bin/env bash

set -euo pipefail

CMD=$1
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

source "$SCRIPT_DIR/sql-test-helpers.sh"

echo "--- $0 start ---"

$CMD "CREATE TABLE items (id INT, name VARCHAR, category TINYINT, height INT, weight INT);"
$CMD "INSERT INTO items (id, name, category, height, weight) VALUES \
  (1, 'TV', 1, 10, 100), \
  (2, 'Chair', 1, 15, 20), \
  (3, 'Light', 2, 5, 50), \
  (4, 'Table', 2, 8, 60) \
;"

SQL_CASES=(
  "SELECT count(1) FROM items"
  "SELECT category, min(height), max(weight) FROM items GROUP BY category"
  "SELECT category, sum(weight) FROM items GROUP BY category HAVING sum(weight) > 115"
)
#
EXPECTED_CASES=(
$'count(1)
4'
$'category	min(height)	max(weight)
1	10	100
2	5	60'
$'category	sum(weight)
1	120'
)

run_sql_cases "$CMD" SQL_CASES EXPECTED_CASES

$CMD "DROP TABLE items;"
echo "--- $0 finish ---"
