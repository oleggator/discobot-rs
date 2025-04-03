build-native:
	cargo build -r

build-aarch64:
	RUSTFLAGS='-Clinker=aarch64-linux-gnu-gcc' CARGO_BUILD_TARGET='aarch64-unknown-linux-gnu' cargo build -r
