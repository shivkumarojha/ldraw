APP_NAME := ldraw
DESKTOP_ID := io.ldraw.LDraw

PREFIX ?= /usr/local
BINDIR ?= $(PREFIX)/bin
APPDIR ?= /usr/share/applications
ICONDIR ?= /usr/share/icons/hicolor/scalable/apps

TARGET := target/release/$(APP_NAME)
DESKTOP_FILE := packaging/$(DESKTOP_ID).desktop
ICON_FILE := assets/ldraw.svg

.PHONY: help run release check install uninstall clean

help:
	@printf "Targets:\n"
	@printf "  make run       - Run in dev mode\n"
	@printf "  make release   - Build release binary\n"
	@printf "  make check     - Format + check + test\n"
	@printf "  sudo make install   - System-wide install + desktop entry\n"
	@printf "  sudo make uninstall - Remove system-wide install\n"

run:
	cargo run

release:
	cargo build --release

check:
	cargo fmt --all
	cargo check
	cargo test

install: release
	install -Dm755 "$(TARGET)" "$(DESTDIR)$(BINDIR)/$(APP_NAME)"
	install -Dm644 "$(DESKTOP_FILE)" "$(DESTDIR)$(APPDIR)/$(DESKTOP_ID).desktop"
	install -Dm644 "$(ICON_FILE)" "$(DESTDIR)$(ICONDIR)/ldraw.svg"
	@if [ -z "$(DESTDIR)" ]; then \
		if command -v update-desktop-database >/dev/null 2>&1; then \
			update-desktop-database "$(APPDIR)" || true; \
		fi; \
		if command -v gtk-update-icon-cache >/dev/null 2>&1; then \
			gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor || true; \
		fi; \
	fi

uninstall:
	rm -f "$(DESTDIR)$(BINDIR)/$(APP_NAME)"
	rm -f "$(DESTDIR)$(APPDIR)/$(DESKTOP_ID).desktop"
	rm -f "$(DESTDIR)$(ICONDIR)/ldraw.svg"
	@if [ -z "$(DESTDIR)" ]; then \
		if command -v update-desktop-database >/dev/null 2>&1; then \
			update-desktop-database "$(APPDIR)" || true; \
		fi; \
		if command -v gtk-update-icon-cache >/dev/null 2>&1; then \
			gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor || true; \
		fi; \
	fi

clean:
	cargo clean
