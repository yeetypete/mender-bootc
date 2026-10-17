# mender-bootc developer tasks.

# List available recipes.
default:
    @just --list

# Check formatting and lint.
check:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets --all-features --locked

# Run the tests.
test:
    cargo test --locked

# Run the integration tests.
test-integration *args:
    cargo xtask test {{ args }}

# Build a release binary.
build:
    cargo build --release --locked
