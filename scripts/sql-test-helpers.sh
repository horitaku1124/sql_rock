#!/usr/bin/env bash

run_sql_cases() {
  local cmd="$1"
  local sql_cases_name="$2"
  local expected_cases_name="$3"
  local sql_cases_length
  local expected_cases_length
  eval "sql_cases_length=\${#$sql_cases_name[@]}"
  eval "expected_cases_length=\${#$expected_cases_name[@]}"

  if [[ "$sql_cases_length" -ne "$expected_cases_length" ]]; then
    printf '%s and %s must have the same length\n' "$2" "$3"
    return 1
  fi

  for ((i = 0; i < sql_cases_length; i++)); do
    local sql
    local output
    local expected
    eval "sql=\${$sql_cases_name[$i]}"
    eval "expected=\${$expected_cases_name[$i]}"
    output="$($cmd "$sql")"

    if [[ "$output" != "$expected" ]]; then
      printf 'failed: %s\nexpected:\n%s\nactual:\n%s\n' \
        "$sql" "$expected" "$output"
      return 1
    fi
  done
}

run_sql_error_cases() {
  local cmd="$1"
  local sql_cases_name="$2"
  local expected_cases_name="$3"
  local sql_cases_length
  local expected_cases_length
  eval "sql_cases_length=\${#$sql_cases_name[@]}"
  eval "expected_cases_length=\${#$expected_cases_name[@]}"

  if [[ "$sql_cases_length" -ne "$expected_cases_length" ]]; then
    printf '%s and %s must have the same length\n' "$2" "$3"
    return 1
  fi

  for ((i = 0; i < sql_cases_length; i++)); do
    local sql
    local output
    local expected
    eval "sql=\${$sql_cases_name[$i]}"
    eval "expected=\${$expected_cases_name[$i]}"

    if output="$($cmd "$sql" 2>&1)"; then
      printf 'failed: %s\nexpected command to fail, but it succeeded with:\n%s\n' \
        "$sql" "$output"
      return 1
    fi

    if [[ "$output" != "$expected" ]]; then
      printf 'failed: %s\nexpected error:\n%s\nactual:\n%s\n' \
        "$sql" "$expected" "$output"
      return 1
    fi
  done
}
