#!/bin/sh
# Install a package from packaging/out in a clean container: install-test.sh IMAGE PACKAGE
set -e
img=$1; pkg=$2
docker run --rm -v "$(dirname "$(readlink -f "$0")")/out:/out:ro" "$img" sh -c '
  pkg=/out/'"$pkg"'
  if command -v apt-get >/dev/null; then
    apt-get update -qq >/dev/null && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq "$pkg" >/tmp/log 2>&1 || { tail -20 /tmp/log; exit 1; }
    dpkg -L shellman | grep -E "\.so$|\.mo$|metainfo"
  elif command -v dnf >/dev/null; then
    dnf install -y -q "$pkg" >/tmp/log 2>&1 || { tail -20 /tmp/log; exit 1; }
    rpm -ql shellman | grep -E "\.so$|\.mo$|metainfo"
  else
    zypper --non-interactive --no-gpg-checks install --allow-unsigned-rpm "$pkg" >/tmp/log 2>&1 || { tail -20 /tmp/log; exit 1; }
    rpm -ql shellman | grep -E "\.so$|\.mo$|metainfo"
  fi'
