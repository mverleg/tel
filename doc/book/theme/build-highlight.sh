#!/usr/bin/env bash
# Regenerate theme/highlight.js = mdBook's default hljs bundle + the Tel language.
#
# highlight.base.js is the pristine bundle mdBook 0.4.x ships (hljs 10.1.1).
# tel-hljs.js adds `hljs.registerLanguage("tel", …)`. mdBook copies the
# resulting theme/highlight.js over its built-in default, so all the stock
# languages stay available and `tel` is added on top.
set -euo pipefail
cd "$(dirname "$0")"
cat highlight.base.js tel-hljs.js > highlight.js
echo "Wrote theme/highlight.js ($(wc -c < highlight.js) bytes)"
