#!/usr/bin/env sh
# Stands in for a build. The fixture exercises composition, not compilers.
echo "build: $(find src -name '*.rs' | wc -l) file(s)"
