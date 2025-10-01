#!/bin/bash
set -e

# Deploy wikiwalk binaries from /tmp to /usr/local/bin
# This script is meant to be run with sudo by the wikiwalk user during deployment

# Deploy tool if it exists
if [ -f /tmp/wikiwalk-tool.tmp ]; then
    mv /tmp/wikiwalk-tool.tmp /usr/local/bin/wikiwalk-tool
    chown root:root /usr/local/bin/wikiwalk-tool
    chmod 755 /usr/local/bin/wikiwalk-tool
    echo "Deployed wikiwalk-tool"
fi

# Deploy server if it exists
if [ -f /tmp/wikiwalk.tmp ]; then
    mv /tmp/wikiwalk.tmp /usr/local/bin/wikiwalk
    chown root:root /usr/local/bin/wikiwalk
    chmod 755 /usr/local/bin/wikiwalk
    setcap cap_net_bind_service+eip /usr/local/bin/wikiwalk
    echo "Deployed wikiwalk server"
fi
