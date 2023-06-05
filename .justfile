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
issue-tls-cert: install-lego
  sudo /usr/local/bin/lego --path /var/wikiwalk/certs --email="LiamBowen@gmail.com" --domains="wikiwalk.app" --key-type ec256 --http run

# Provision server (create speedrun user)
provision-server:
  sudo adduser --home /home/wikiwalk --shell /bin/bash --gecos 'WikiWalk' --disabled-password wikiwalk
  sudo apt-get install -y pkg-config

# Deploy web server (must be run on server)
deploy-web: build-release
  sudo mkdir -p /var/wikiwalk/ /var/wikiwalk/data /var/wikiwalk/webroot/.well-known
  sudo chown -R wikiwalk:wikiwalk /var/wikiwalk
  sudo rm -f /usr/local/bin/wikiwalk
  sudo rm -f /usr/local/bin/wikiwalk-watchdog
  sudo rm -f /usr/local/bin/wikiwalk-monitor
  sudo rm -f /usr/local/bin/wikiwalk-certs
  sudo cp target/release/server /usr/local/bin/wikiwalk
  sudo cp wikiwalk-watchdog /usr/local/bin/wikiwalk-watchdog
  sudo cp wikiwalk-monitor /usr/local/bin/wikiwalk-monitor
  sudo cp wikiwalk-certs /usr/local/bin/wikiwalk-certs
  sudo setcap cap_net_bind_service+eip /usr/local/bin/wikiwalk
  sudo cp ./wikiwalk.service /lib/systemd/system/wikiwalk.service
  sudo cp ./wikiwalk-certs.service /lib/systemd/system/wikiwalk-certs.service
  sudo cp ./wikiwalk-certs.timer /lib/systemd/system/wikiwalk-certs.timer
  sudo cp ./wikiwalk-watchdog.service /lib/systemd/system/wikiwalk-watchdog.service
  sudo systemctl daemon-reload
  sudo systemctl enable wikiwalk.service
  sudo systemctl enable wikiwalk-certs.service
  sudo systemctl enable wikiwalk-certs.timer
  sudo systemctl enable wikiwalk-watchdog.service
  sudo systemctl restart wikiwalk.service
  sudo systemctl restart wikiwalk-certs.timer
  sudo systemctl restart wikiwalk-watchdog.service

# Deploy wikiwalk tool and periodic builds
deploy-tool: build-release-tool
  sudo mkdir -p /var/wikiwalk/ /var/wikiwalk/data /var/wikiwalk/webroot/.well-known
  sudo chown -R wikiwalk:wikiwalk /var/wikiwalk
  sudo rm -f /usr/local/bin/wikiwalk-tool
  sudo cp target/release/tool /usr/local/bin/wikiwalk-tool
  sudo cp ./wikiwalk-build.timer /lib/systemd/system/wikiwalk-build.timer
  sudo cp ./wikiwalk-build.service /lib/systemd/system/wikiwalk-build.service
  sudo systemctl daemon-reload
  sudo systemctl enable wikiwalk-build.service
  sudo systemctl enable wikiwalk-build.timer
  sudo systemctl start wikiwalk-build.service
  sudo systemctl start wikiwalk-build.timer

# Deploy configuration file shared by tool and server
deploy-config:
  sudo rm -f /etc/wikiwalk.conf  
  sudo cp wikiwalk.conf /etc/wikiwalk.conf

# Deploy web server and tool
deploy: deploy-config deploy-tool deploy-web
