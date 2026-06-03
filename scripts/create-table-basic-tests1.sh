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
$CMD "INSERT INTO items (name) VALUES \
  ('TV'), \
  ('Chair') \
;"

SQL_CASES=(
  "SELECT count(1) FROM items"
)
#
EXPECTED_CASES=(
$'count(1)
3'
)


$CMD "DROP TABLE items;"
echo "--- $0 finish ---"
