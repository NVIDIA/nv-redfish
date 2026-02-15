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
                   assembly \
                   bios \
                   boot-options \
                   chassis \
                   computer-systems \
                   ethernet-interfaces \
                   log-services \
                   memory \
                   network-adapters \
                   power \
                   power-supplies \
                   processors \
                   secure-boot \
                   sensors \
                   storages \
                   thermal \
                   update-service

# Features that cannot be compiled standalone (no references from the tree).
std-not-standalone-features = assembly \
             bios \
             boot-options \
             ethernet-interfaces \
             log-services \
             network-adapters \
             processors \
             power \
             power-supplies \
             secure-boot \
             sensors \
             storages

std-standalone-features = $(filter-out $(std-not-standalone-features),$(all-std-features))

ci-features-list := $(subst $(space),$(comma),$(all-std-features))

compile-one-feature = $(indent)cargo build -p nv-redfish --features $1$(new-line)

define build-and-test
	cargo build
	cargo build -p nv-redfish
	cargo build -p nv-redfish-tests --tests
	cargo build -p nv-redfish-bmc-mock
	cargo test $1 -- --no-capture
	cargo clippy $1
	cargo build  $1
	cargo build -p nv-redfish --features computer-systems,bios,boot-options,storages,memory,processors
	cargo build -p nv-redfish --features oem-hpe,accounts
	$(maybe-lenovo-build)
	cargo build -p nv-redfish --features oem-hpe
	cargo build -p nv-redfish --features oem-nvidia
	cargo build -p nv-redfish --features computer-systems,oem-nvidia-bluefield
	cargo build -p nv-redfish --features oem-dell
	cargo build -p nv-redfish --features oem-ami
	$(foreach f,$(std-standalone-features),$(call compile-one-feature,$f))
	cargo build -p nv-redfish --features ""
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


