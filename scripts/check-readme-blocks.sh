#!/usr/bin/env bash
# Validate every fenced ```json block in README.md with `jq`, so a documented
# JSON shape can't silently drift into invalid JSON as the README is edited.
set -euo pipefail

readme="README.md"
status=0
in_block=0
block=""
start_line=0
lineno=0

while IFS= read -r line || [[ -n "$line" ]]; do
  lineno=$((lineno + 1))
  if [[ $in_block -eq 0 ]]; then
    if [[ $line == '```json' ]]; then
      in_block=1
      block=""
      start_line=$lineno
    fi
  elif [[ $line == '```' ]]; then
    in_block=0
    if ! jq -e . >/dev/null 2>/tmp/readme-block-err <<<"$block"; then
      echo "::error file=$readme,line=$start_line::json block is not valid JSON: $(cat /tmp/readme-block-err)"
      status=1
    fi
  else
    block+="$line"$'\n'
  fi
done <"$readme"

rm -f /tmp/readme-block-err
exit "$status"
