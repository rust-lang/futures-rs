#!/bin/bash

# Format all rust code.
#
# Usage:
#    ./tools/fmt.sh
#
# This is similar to `cargo fmt`, but unlike `cargo fmt`, it can recognize
# modules defined inside macros.
# Refs: https://github.com/rust-lang/rustfmt/issues/4078

set -euo pipefail
IFS=$'\n\t'

cd "$(cd "$(dirname "${0}")" && pwd)"/..

# shellcheck disable=SC2046
if [[ -z "${CI:-}" ]]; then
    # `cargo fmt` cannot recognize modules defined inside macros, so run
    # rustfmt directly.
    # Refs: https://github.com/rust-lang/rustfmt/issues/4078
    rustfmt $(git ls-files "*.rs")
else
    # `cargo fmt` cannot recognize modules defined inside macros, so run
    # rustfmt directly.
    # Refs: https://github.com/rust-lang/rustfmt/issues/4078
    rustfmt --check $(git ls-files "*.rs")
fi
