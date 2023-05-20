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
  rm -f target/release/server target/release/tool
  cargo build --release --bin server

build-release-tool:
    rm -f target/release/tool
    cargo build --release --bin tool

# Build for development
build:
  cargo build

# Install lego for TLS certificates
install-lego:
  go install github.com/go-acme/lego/v4/cmd/lego@latest
  sudo mv "$HOME/go/bin/lego" /usr/local/bin/lego

# Get production TLS certificate once
issue-tls-cert:
  sudo /usr/local/bin/lego --path /var/wikipedia-speedrun/certs --email="LiamBowen@gmail.com" --domains="wikipediaspeedrun.com" --key-type ec256 --http run

# Provision server (create speedrun user)
provision-server:
  sudo adduser --home /home/speedrun --shell /bin/bash --gecos 'Wikipedia Speedrun' --disabled-password speedrun
  sudo apt-get install -y pkg-config

# Deploy web server (must be run on server)
deploy-web: build-release
  sudo mkdir -p /var/wikipedia-speedrun/ /var/wikipedia-speedrun/webroot/.well-known
  sudo chown -R speedrun:speedrun /var/wikipedia-speedrun
  sudo rm -f /usr/local/bin/wikipedia-speedrun
  sudo rm -f /usr/local/bin/wikipedia-speedrun-watchdog
  sudo rm -f /usr/local/bin/wikipedia-speedrun-monitor
  sudo rm -f /usr/local/bin/wikipedia-speedrun-certs
  sudo rm -f /etc/wikipedia-speedrun.conf
  sudo cp target/release/server /usr/local/bin/wikipedia-speedrun
  sudo cp wikipedia-speedrun-watchdog /usr/local/bin/wikipedia-speedrun-watchdog
  sudo cp wikipedia-speedrun-monitor /usr/local/bin/wikipedia-speedrun-monitor
  sudo cp wikipedia-speedrun-certs /usr/local/bin/wikipedia-speedrun-certs
  sudo setcap cap_net_bind_service+eip /usr/local/bin/wikipedia-speedrun
  sudo cp ./wikipedia-speedrun.service /lib/systemd/system/wikipedia-speedrun.service
  sudo cp ./wikipedia-speedrun-certs.service /lib/systemd/system/wikipedia-speedrun-certs.service
  sudo cp ./wikipedia-speedrun-certs.timer /lib/systemd/system/wikipedia-speedrun-certs.timer
  sudo cp ./wikipedia-speedrun-watchdog.service /lib/systemd/system/wikipedia-speedrun-watchdog.service
  sudo systemctl daemon-reload
  sudo systemctl enable wikipedia-speedrun.service
  sudo systemctl enable wikipedia-speedrun-certs.service
  sudo systemctl enable wikipedia-speedrun-certs.timer
  sudo systemctl enable wikipedia-speedrun-watchdog.service
  sudo systemctl restart wikipedia-speedrun.service
  sudo systemctl restart wikipedia-speedrun-certs.timer
  sudo systemctl restart wikipedia-speedrun-watchdog.service

# Deploy wikipedia-speedrun tool and periodic builds
deploy-tool: build-release-tool
  sudo rm -f /usr/local/bin/wikipedia-speedrun-tool
  sudo cp target/release/tool /usr/local/bin/wikipedia-speedrun-tool
  sudo cp ./wikipedia-speedrun-build.timer /lib/systemd/system/wikipedia-speedrun-build.timer
  sudo cp ./wikipedia-speedrun-build.service /lib/systemd/system/wikipedia-speedrun-build.service
  sudo systemctl daemon-reload
  sudo systemctl enable wikipedia-speedrun-build.service
  sudo systemctl enable wikipedia-speedrun-build.timer
  sudo systemctl start wikipedia-speedrun-build.service
  sudo systemctl start wikipedia-speedrun-build.timer

# Deploy configuration file shared by tool and server
deploy-config:
  sudo rm -f /etc/wikipedia-speedrun.conf  
  sudo cp wikipedia-speedrun.conf /etc/wikipedia-speedrun.conf

# Deploy web server and tool
deploy: deploy-config deploy-tool deploy-web
