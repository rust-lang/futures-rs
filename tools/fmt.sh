#!/bin/bash

# Format all rust code.
#
# Usage:
#    ./tools/fmt.sh
#
# This script is needed because `cargo fmt` cannot recognize modules defined inside macros.
# Refs: https://github.com/rust-lang/rustfmt/issues/4078

set -euo pipefail
IFS=$'\n\t'

cd "$(cd "$(dirname "${0}")" && pwd)"/..

# shellcheck disable=SC2046
if [[ -z "${CI:-}" ]]; then
    (
        # `cargo fmt` cannot recognize modules defined inside macros so run rustfmt directly.
        rustfmt $(git ls-files "*.rs")
    )
else
    (
        rustfmt --check $(git ls-files "*.rs")
    )
fi
