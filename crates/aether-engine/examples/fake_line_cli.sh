#!/usr/bin/env sh
# A stand-in for a third-party engine CLI, for `line_engine` and its test.
# Speaks newline-delimited JSON on stdout and reads Aether's answers on stdin.
set -eu
read -r _prompt
printf '%s\n' '{"type":"session","id":"sess-1","model":"demo"}'
printf '%s\n' '{"type":"text","text":"Listing the workspace."}'
printf '%s\n' '{"type":"permission","id":"p1","tool":"bash","input":{"command":"ls"}}'
read -r answer
case "$answer" in
  *'"allow":true'*) printf '%s\n' '{"type":"tool_result","id":"p1","content":"README.md","error":false}' ;;
  *)                printf '%s\n' '{"type":"tool_result","id":"p1","content":"refused","error":true}' ;;
esac
printf '%s\n' '{"type":"done","text":"Done.","session":"sess-1","input_tokens":11,"output_tokens":4}'
