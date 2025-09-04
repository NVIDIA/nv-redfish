#
# Just helper to run all tests
#

all:
	cargo build
	cargo test
	cargo clippy
	cargo doc

