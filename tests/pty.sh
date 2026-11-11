#!/bin/sh
# bash / zsh / fish integration in a real terminal: ./tests/pty.sh
set -e
cd "$(dirname "$0")"
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
echo "=== bash"; python3 pty_test_bash.py "$T/bash" "trap 'echo x >> $T/bash/dbg' DEBUG" 2>&1 | grep -v 'Errno 5' || true
for sh in zsh fish; do
    command -v $sh >/dev/null || { echo "=== $sh: not installed"; continue; }
    echo "=== $sh"; python3 pty_test_shells.py $sh "$T/$sh" 2>&1 | grep -v 'Errno 5' || true
done
