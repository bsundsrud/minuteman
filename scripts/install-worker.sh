#!/bin/sh
set -e

COORDINATOR_URL="$1"
if [ -z "$COORDINATOR_URL" ]; then
    echo "Usage: install-worker.sh COORDINATOR_URL [INSTALL_PREFIX]"
    exit 1
fi

INSTALL_PREFIX="$2"
if [ -z "$INSTALL_PREFIX" ]; then
    INSTALL_PREFIX="/usr/local"
fi

install bin/minuteman "${INSTALL_PREFIX}/bin"

sed "s^##INSTALL_DIR##^${INSTALL_PREFIX}/bin^" systemd/minuteman-worker.service > /etc/systemd/system/minuteman-worker.service

sed "s^##COORDINATOR_URL##^${COORDINATOR_URL}^" systemd/minuteman-worker.defaults > /etc/default/minuteman-worker

echo "Installation complete, start with 'systemctl start minuteman-worker'"
