#!/bin/sh
set -e

INSTALL_PREFIX="$1"
if [ -z "$INSTALL_PREFIX" ]; then
    INSTALL_PREFIX="/usr/local"
fi

install bin/minuteman "${INSTALL_PREFIX}/bin"

sed "s^##INSTALL_DIR##^${INSTALL_PREFIX}/bin^" systemd/minuteman-coordinator.service > /etc/systemd/system/minuteman-coordinator.service

echo "Installation complete, start with 'systemctl start minuteman-coordinator'"
