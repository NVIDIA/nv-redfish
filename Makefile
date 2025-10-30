#
# Build and test everything.
#

pwd := $(shell pwd)

maybe-lenovo-build = $(if $(wildcard $(pwd)/oem/lenovo/*.xml),cargo build --features oem-lenovo)

space := $(empty) $(empty)
comma :=,
indent := $(empty)	$(empty)
define new-line
$(empty)
$(empty)
endef

# We cannot use --all-features because they depends on oem files that
# are not distributed by the repo.
all-std-features = accounts \
                   chassis \
                   log-services \
                   memory \
                   power-supplies \
                   processors \
                   sensors \
                   storages \
                   systems \
                   update-service \
# Features that cannot be compiled standalone (no references from the tree).
std-not-standalone-features = log-services sensors
std-standalone-features = $(filter-out $(std-not-standalone-features),$(all-std-features))

ci-features-list := $(subst $(space),$(comma),$(all-std-features))

compile-one-feature = $(indent)cargo build --features $1$(new-line)

define build-and-test
	cargo build
	cargo test $1 -- --no-capture
	cargo clippy $1
	cargo build  $1
	cargo build --features oem-hpe,accounts
	$(maybe-lenovo-build)
	cargo build --features oem-hpe
	cargo build --features oem-nvidia
	cargo build --features oem-dell
	cargo build --features oem-ami
	$(foreach f,$(std-standalone-features),$(call compile-one-feature,$f))
	cargo build --features ""
	cargo doc $1
	cargo build

endef


all:
	$(call build-and-test,--all-features)

ci: rust-install
	$(call build-and-test,--features $(ci-features-list))

rust-install:
	rustup component add clippy

clean:
	rm -rf $(schema-dir)
	rm -rf target


