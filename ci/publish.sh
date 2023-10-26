#!/bin/bash

set -euo pipefail
IFS=$'\n\t'
cd "$(dirname "$0")"/..

# A list of paths to the crate to be published.
# It will be published in the order listed.
members=(
    "futures-core"
    "futures-io"
    "futures-sink"
    "futures-task"
    "futures-channel"
    "futures-macro"
    "futures-util"
    "futures-executor"
    "futures"
    "futures-test"
)

for member in "${members[@]}"; do
    (
        set -x
        cd "${member}"
        cargo +stable publish
    )
done
