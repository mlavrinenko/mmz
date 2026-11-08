#!/usr/bin/env sh
# Stands in for a build.
echo "build: $(find src -name '*.rs' | wc -l) file(s)"
