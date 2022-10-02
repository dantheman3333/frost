#!/usr/bin/env bash
set -e

script_dir="${0%/*}"
cd "${script_dir}"
cd ../../../ # repo root

FILEPATH=./examples/read_bag/fixtures/test.bag

source ./scripts/setup_py.sh
PYTHON=$(get_python)
setup_venv

$PYTHON ./examples/read_bag/scripts/gen.py --output "$FILEPATH" --count 100
