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
                   managers \
                   manager-network-protocol \
                   memory \
                   network-adapters \
                   power \
                   power-equipment \
                   power-supplies \
                   processors \
                   secure-boot \
                   sensors \
                   session-service \
                   storages \
                   thermal \
                   update-service \
                   event-service \
                   task-service

# Features that cannot be compiled standalone (no references from the tree).
std-not-standalone-features = assembly \
             bios \
             boot-options \
             ethernet-interfaces \
             log-services \
             manager-network-protocol \
             network-adapters \
             processors \
             power \
             power-supplies \
             secure-boot \
             sensors \
             storages \
             update-service-deprecated

std-standalone-features = $(filter-out $(std-not-standalone-features),$(all-std-features))

ci-features-list := $(subst $(space),$(comma),$(all-std-features))

compile-one-feature = $(indent)cargo build -p nv-redfish --features $1$(new-line)

define build-and-test
	cargo fmt --all -- --check
	cargo clippy $1
	cargo build -p nv-redfish --features computer-systems,processors,controls
	cargo build -p nv-redfish --features managers,oem-hpe
	cargo build -p nv-redfish --features managers,oem-supermicro
	cargo build -p nv-redfish --features chassis,power-supplies,oem-liteon
	cargo build -p nv-redfish --features chassis,controls
	cargo build
	cargo build -p nv-redfish
	cargo build -p nv-redfish-tests --tests
	cargo build -p nv-redfish-bmc-mock
	cargo test $1 -- --no-capture
	cargo build -p nv-redfish --features update-service-deprecated
	cargo build -p nv-redfish --features bmc-http,update-service-deprecated
	cargo test -p nv-redfish-bmc-http --test reqwest_client_tests --features reqwest,update-service-deprecated
	cargo test -p nv-redfish-tests --test test-update-service --features update-service-deprecated
	cargo build -p update-multipart --features update-service-deprecated
	cargo clippy -p nv-redfish-dispatcher --all-targets
	cargo clippy -p nv-redfish-dispatcher --all-targets --all-features
	cargo test -p nv-redfish-dispatcher --all-features -- --no-capture
	cargo clippy -p nv-redfish-bmc-http --bench cache
	cargo build  $1
	cargo build -p nv-redfish --features computer-systems,bios,boot-options,storages,memory,processors
	cargo build -p nv-redfish --features oem-hpe,accounts
	$(maybe-lenovo-build)
	cargo build -p nv-redfish --features oem-hpe
	cargo build -p nv-redfish --features oem-nvidia
	cargo build -p nv-redfish --features computer-systems,oem-nvidia-bluefield
	cargo build -p nv-redfish --features oem-dell
	cargo build -p nv-redfish --features oem-ami
	cargo build -p nv-redfish --features managers,oem-dell-attributes
	$(foreach f,$(std-standalone-features),$(call compile-one-feature,$f))
	cargo build -p nv-redfish --features ""
	cargo doc --no-deps $1
	cargo build

endef


all:
	$(call build-and-test,--all-features)

ci: rust-install
	$(call build-and-test,--features $(ci-features-list))

rust-install:
	rustup component add clippy rustfmt

clean:
	rm -rf $(schema-dir)
	rm -rf target
