# Release build and install, used by the deb and rpm packages.
# Development: ./install.sh
PREFIX ?= /usr
LOCALEDIR ?= $(PREFIX)/share/locale
EXTENSIONDIR ?= $(shell pkg-config --variable=extensiondir libnautilus-extension-4)
CARGO ?= cargo
TARGET := target/package
LANGS := $(shell grep -v '^\#' po/LINGUAS)
METAINFO := io.github.scresto09.ShellMan.metainfo.xml

all:
	SHELLMAN_LOCALEDIR=$(LOCALEDIR) $(CARGO) build --release --locked --target-dir $(TARGET)
	for lang in $(LANGS); do \
		mkdir -p $(TARGET)/locale/$$lang/LC_MESSAGES && \
		msgfmt --check -o $(TARGET)/locale/$$lang/LC_MESSAGES/shellman.mo po/$$lang.po || exit 1; \
	done
	msgfmt --xml -d po --template data/$(METAINFO) -o $(TARGET)/$(METAINFO)

install:
	@test -n "$(EXTENSIONDIR)" || { echo "libnautilus-extension-4 not found by pkg-config" >&2; exit 1; }
	install -Dm644 $(TARGET)/release/libshellman.so $(DESTDIR)$(EXTENSIONDIR)/libnautilus-shellman.so
	install -Dm644 $(TARGET)/$(METAINFO) $(DESTDIR)$(PREFIX)/share/metainfo/$(METAINFO)
	for lang in $(LANGS); do \
		install -Dm644 $(TARGET)/locale/$$lang/LC_MESSAGES/shellman.mo \
			$(DESTDIR)$(LOCALEDIR)/$$lang/LC_MESSAGES/shellman.mo || exit 1; \
	done

check:
	appstreamcli validate --no-net data/$(METAINFO)

test:
	cargo clippy --all-targets -- -D warnings
	cargo test --all-targets
	cargo build --release --locked

clean:
	$(CARGO) clean

.PHONY: all install check test clean
