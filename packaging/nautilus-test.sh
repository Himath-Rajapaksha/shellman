#!/bin/sh
# Start Nautilus with ShellMan in a clean container (Xvfb, or headless mutter), open
# the terminal (SHELLMAN_TEST_SHOW) and check that its shell starts.
# Usage: packaging/nautilus-test.sh IMAGE PACKAGE   (PACKAGE in packaging/out)
set -eu
if [ "${1:-}" = --inside ]; then
	pkg=$2
	if command -v apt-get >/dev/null; then
		export DEBIAN_FRONTEND=noninteractive
		apt-get update -qq
		apt-get install -y -qq "$pkg" nautilus xvfb xauth dbus-x11 procps >/dev/null 2>&1
	elif command -v dnf >/dev/null; then
		dnf install -y -q "$pkg" nautilus dbus-daemon procps-ng >/dev/null 2>&1
		# EL10 has no Xvfb: headless mutter instead
		dnf install -y -q xorg-x11-server-Xvfb xauth >/dev/null 2>&1 ||
			dnf install -y -q mutter >/dev/null 2>&1
	else
		zypper -n -q --no-gpg-checks install --allow-unsigned-rpm "$pkg" nautilus xvfb-run dbus-1-daemon procps >/dev/null 2>&1
	fi
	# Nautilus refuses to run as root
	useradd -m -s /bin/bash tester
	mkdir -p -m 700 /tmp/run
	chown tester /tmp/run
	cat > /tmp/run.sh <<'EOT'
if [ -z "${DISPLAY:-}" ]; then
	mutter --headless --wayland --no-x11 --virtual-monitor 1280x800 > /tmp/mutter.log 2>&1 &
	sleep 5
	export WAYLAND_DISPLAY=wayland-0
fi
nautilus /tmp > /tmp/nautilus.log 2>&1 &
sleep 15
echo "--- [shellman] log"
grep -E 'shellman|panic|abort' /tmp/nautilus.log || echo "(nothing)"
echo "--- shell started by the terminal"
pid=$(pgrep -xo nautilus)
[ -n "$pid" ] && pgrep -P "$pid" -a || echo "NONE"
nautilus -q 2>/dev/null
sleep 2
pkill -u "$(id -u)" -f -v run.sh 2>/dev/null
EOT
	chmod 644 /tmp/run.sh
	set --
	command -v xvfb-run >/dev/null && set -- xvfb-run -a -s "-screen 0 1280x800x24"
	exec runuser -u tester -- env HOME=/home/tester GSK_RENDERER=cairo LANG=C.UTF-8 \
		XDG_RUNTIME_DIR=/tmp/run SHELLMAN_DUMP=1 SHELLMAN_TEST_SHOW=1 \
		dbus-run-session -- "$@" sh /tmp/run.sh
fi
image=$1
pkg=$2
cd "$(dirname "$0")"
timeout 900 docker run --rm -v "$PWD:/packaging:ro" "$image" sh /packaging/nautilus-test.sh --inside "/packaging/out/$pkg"
