#
# Just helper to run all tests
#

all:
	cargo build
	cargo test -- --no-capture
	cargo clippy
	cargo doc

