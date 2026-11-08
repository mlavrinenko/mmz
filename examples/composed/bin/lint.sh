#!/usr/bin/env sh
# Stands in for a linter.
echo "lint: $(find tests -name '*.rs' | wc -l) file(s)"
