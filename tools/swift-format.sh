#!/usr/bin/env bash
# dprint's Swift formatter (called from the `exec` plugin with no arguments;
# stdin carries the file and the formatted result is printed to stdout). The
# settings live in `.swift-format`.
set -euo pipefail

exec swift-format format -
