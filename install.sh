#!/bin/sh
# Development install of ShellMan.
#
# Nautilus only loads native extensions from its system folder: this puts
# (with sudo, once) a link there to the .so built here. After that,
# cargo build --release is enough, then nautilus -q && nautilus.
#
#   ./install.sh            build, and create the link if needed
#   ./install.sh --remove   remove the link
set -e
cd "$(dirname "$0")"
DIR=/usr/lib/x86_64-linux-gnu/nautilus/extensions-4
LINK=$DIR/libnautilus-shellman.so
SO=$(pwd)/target/release/libshellman.so

if [ "$1" = "--remove" ]; then
    sudo rm -f "$LINK" && echo "removed: $LINK"
    exit 0
fi

cargo build --release
if [ "$(readlink "$LINK" 2>/dev/null)" != "$SO" ]; then
    sudo ln -sf "$SO" "$LINK"
    echo "link created: $LINK -> $SO"
fi
echo "restart Nautilus: nautilus -q && nautilus"
