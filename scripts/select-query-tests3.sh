#!/usr/bin/env bash

set -euo pipefail

CMD=$1
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

source "$SCRIPT_DIR/sql-test-helpers.sh"

echo "--- $0 start ---"

$CMD "CREATE TABLE users (id INT, name VARCHAR, age BIGINT);"
$CMD "INSERT INTO users (id, name, age) VALUES (1, 'Alice', 15), (2, 'Bob', 20), (3, 'Noice', 18);"

SQL_CASES=(
  "SELECT * FROM users ORDER BY id;"
  "SELECT max(age) FROM users;"
  "SELECT min(age) FROM users;"
  "SELECT avg(age) FROM users;"
  "SELECT sum(age) FROM users;"
)
#
EXPECTED_CASES=(
$'id	name	age
1	Alice	15
2	Bob	20
3	Noice	18'

$'max(age)\n20'
$'min(age)\n15'
$'avg(age)\n17.666666666666668'
$'sum(age)\n53'
)

run_sql_cases "$CMD" SQL_CASES EXPECTED_CASES

$CMD "DROP TABLE users;"
echo "--- $0 finish ---"
