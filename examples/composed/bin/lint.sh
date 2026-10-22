#!/usr/bin/env sh
# Stands in for a linter over the fragment's own scope.
echo "lint: $(find tests -name '*.rs' | wc -l) file(s)"
