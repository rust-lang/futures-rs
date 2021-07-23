#!/bin/bash

set -euo pipefail
IFS=$'\n\t'

# A list of paths to the crate to be published.
# It will be published in the order listed.
MEMBERS=(
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

cd "$(cd "$(dirname "$0")" && pwd)"/..

set -x

for i in "${!MEMBERS[@]}"; do
    (
        cd "${MEMBERS[${i}]}"
        cargo publish
    )
    if [[ $((i + 1)) != "${#MEMBERS[@]}" ]]; then
        sleep 45
    fi
done
