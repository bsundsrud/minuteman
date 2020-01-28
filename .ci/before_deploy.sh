#!/bin/sh
is_linux() {
    case "$TRAVIS_OS_NAME" in
        linux) return 0 ;;
        *)   return 1 ;;
    esac
}

if is_linux; then
    rustup target add x86_64-unknown-linux-musl
fi

make init build-prod dist VERSION="$TRAVIS_TAG" TARGET="$TARGET"
