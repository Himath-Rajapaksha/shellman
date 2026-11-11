#!/bin/sh
# Build the package on a clean system, as root: installs the build dependencies
# and a Rust toolchain (rustup: distribution versions are too old on some).
# Normally started by build.sh.
# Usage: build-in-container.sh deb|rpm VERSION RELEASE PACKAGING_DIR OUT_DIR
# OUT_DIR must contain shellman-VERSION.tar.gz.

set -eu

kind=$1
version=$2
release=$3
packaging=$4
out=$5
work=$(mktemp -d)

rust() {
	CARGO_HOME=/opt/cargo RUSTUP_HOME=/opt/rustup
	export CARGO_HOME RUSTUP_HOME
	curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs |
		sh -s -- -y --profile minimal --default-toolchain stable
	PATH=$CARGO_HOME/bin:$PATH
	export PATH
}

case $kind in
  deb)
    export DEBIAN_FRONTEND=noninteractive
    apt-get update
    apt-get install -y --no-install-recommends build-essential dpkg-dev curl ca-certificates
    rust
    tar -xzf "$out/shellman-$version.tar.gz" -C "$work"
    cd "$work/shellman-$version"
    cp -r "$packaging/debian" debian
    chmod +x debian/rules
    debversion=$version-$release
    cat > debian/changelog <<EOT
shellman ($debversion) unstable; urgency=medium

  * Build of ShellMan $version.

 -- Sylvain Cresto <scresto@gmail.com>  $(date -R)
EOT
    apt-get build-dep -y ./
    dpkg-buildpackage -us -uc -b
    cp "$work/shellman_${debversion}_"*.deb "$out/"
    ;;
  rpm)
    if command -v dnf >/dev/null; then
      dnf install -y rpm-build dnf-plugins-core curl
      # The development packages of RHEL 10 and its rebuilds are in CodeReady.
      if rpm -E '%{?rhel}' | grep -q .; then
        dnf config-manager --set-enabled crb
      fi
    else
      zypper --non-interactive install rpm-build curl
    fi
    mkdir -p "$work/SOURCES"
    cp "$out/shellman-$version.tar.gz" "$work/SOURCES/"
    set -- --define "_topdir $work" --define "shellman_version $version" \
	    --define "shellman_release $release" --define "dist %{nil}"
    if command -v dnf >/dev/null; then
      dnf builddep -y "$@" "$packaging/shellman.spec"
    else
      rpmspec -q --buildrequires "$@" "$packaging/shellman.spec" |
	      xargs -d '\n' zypper --non-interactive install
    fi
    rust
    rpmbuild -bb "$@" "$packaging/shellman.spec"
    cp "$work/RPMS/"*/"shellman-$version-"*.rpm "$out/"
    ;;
  *)
    echo "unknown package type: $kind" >&2
    exit 1
    ;;
esac
