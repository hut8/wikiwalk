# just manual: https://github.com/casey/just/#readme

set windows-shell := ["pwsh.exe", "-NoLogo", "-Command"]

_default:
    @just --list

# Set up (and update) tooling
setup:
    # Ignore rustup failures, because not everyone might use it
    rustup self update || true

# Format source code
fmt:
    cargo fmt --all

# Upgrade (and update) dependencies and tools
upgrade: setup
    cargo upgrade
    cargo update

# Fix lint warnings
fix:
    cargo fix --workspace --all-targets
    cargo clippy --workspace --all-targets --fix

# Build for release
build-release:
  cd ui && npm run build
  cargo build --release

# Build for development
build:
  cd ui && npm run build
  cargo build

# Deploy (must be run on server)
deploy: build-release
  rm -rf /var/wikipedia-speedrun/public
  cp -rav ui/dist /var/wikipedia/public
  rm /usr/local/bin/wikipedia-speedrun
  cp target/release/server /usr/local/bin/wikipedia-speedrun
  sudo systemctl restart wikipedia-speedrun
