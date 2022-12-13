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
  cargo build --release --features tls-redirect

# Build for development
build:
  cd ui && npm run build
  cargo build

# Install lego for TLS certificates
install-lego:
  go install github.com/go-acme/lego/v4/cmd/lego@latest
  sudo mv "$HOME/go/bin/lego" /usr/local/bin/lego

# Get production TLS certificate
issue-tls-cert:
  sudo /usr/local/bin/lego --path /var/wikipedia-speedrun/certs --email="LiamBowen@gmail.com" --domains="wikipediaspeedrun.com" --key-type rsa4096 --http run

# Deploy (must be run on server)
deploy: build-release
  rm -rf /var/wikipedia-speedrun/public
  cp -rav ui/dist /var/wikipedia-speedrun/public
  sudo rm -f /usr/local/bin/wikipedia-speedrun
  sudo cp target/release/server /usr/local/bin/wikipedia-speedrun
  sudo setcap cap_net_bind_service+eip /usr/local/bin/wikipedia-speedrun
  sudo systemctl restart wikipedia-speedrun
