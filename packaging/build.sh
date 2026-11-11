#!/bin/sh
# Build a ShellMan package in a container. The result is in packaging/out.
# In a git repository the package is built from a commit (REF, default HEAD):
# uncommitted changes are not included. Otherwise from the working tree.
#
# Usage: packaging/build.sh deb|rpm [IMAGE]
# Environment: REF      commit to build, default HEAD
#              WORKTREE set: build the working tree, uncommitted changes included
#              RELEASE  package revision, default 1
#
# One package of each kind, built on the oldest distribution (Ubuntu 24.04,
# AlmaLinux 10), works on the newer ones.
# Only distributions with Nautilus 43 or later (extension API 4), GTK 4.14 and
# libadwaita 1.5: no txz, Slackware ships neither GNOME nor Nautilus.

set -eu

kind=${1:-}
case $kind in
  deb) image=${2:-ubuntu:24.04} ;;
  rpm) image=${2:-almalinux:10} ;;
  *)
    echo "usage: $0 deb|rpm [image]" >&2
    exit 1
    ;;
esac
ref=${REF:-HEAD}
release=${RELEASE:-1}

cd "$(dirname "$0")/.."
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)

out=$PWD/packaging/out
mkdir -p "$out"
tarball=$out/shellman-$version.tar.gz
if [ -z "${WORKTREE:-}" ] && git rev-parse --verify -q "$ref^{commit}" >/dev/null 2>&1; then
	git archive --format=tar.gz --prefix="shellman-$version/" -o "$tarball" "$ref"
	from=$(git rev-parse --short "$ref")
else
	tar -czf "$tarball" --exclude=./target --exclude=./packaging/out --exclude=./.git \
		--transform "s,^\.,shellman-$version," .
	from="working tree"
fi

echo "Building $kind of ShellMan $version from $from in $image"
# The container runs as root: give the files in packaging/out back to the user.
docker run --rm --entrypoint /bin/sh -v "$PWD/packaging:/packaging:ro" -v "$out:/out" "$image" -c \
	'sh /packaging/build-in-container.sh "$@"; s=$?; chown -R '"$(id -u):$(id -g)"' /out; exit $s' \
	sh "$kind" "$version" "$release" /packaging /out
ls -l "$out"
