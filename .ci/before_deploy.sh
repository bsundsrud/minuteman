#!/bin/sh

make build-prod dist VERSION="$TRAVIS_TAG" TARGET="$TARGET"
