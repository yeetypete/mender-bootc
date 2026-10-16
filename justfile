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

# Build a release binary.
build:
    cargo build --release --locked
