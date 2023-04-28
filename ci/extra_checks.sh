#!/usr/bin/env bash
set -eEu -o pipefail
shopt -s nullglob

is_ok=true
cnt=0
for example_pth in compiler/examples/*.steel; do
    echo "example: $example_pth"
    cargo run -q -- build "$example_pth" || \
        { echo FAILED ; is_ok=false ; }
    cnt=$((cnt+1))
done

if ! $is_ok ; then
    echo "one or more examples failed" 1>&2
    exit 1
fi

if [ "$cnt" -eq "0" ]; then
    echo "no examples were found" 1>&2
    exit 1
fi

echo "all $cnt examples passed"

