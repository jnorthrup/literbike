PREFIX ?= /usr/local
BINARY = literbike-selfhosted

.PHONY: build install

build:
	cargo build --manifest-path tools/selfhosted/Cargo.toml --release

install: build
	install -d $(DESTDIR)$(PREFIX)/bin
	install -m 0755 target/release/$(BINARY) $(DESTDIR)$(PREFIX)/bin/ || true
