#!/usr/bin/env bash
set -e

# Move to the script dir then back to root so this script can be ran from anywhere
script_dir="${0%/*}"
cd "${script_dir}"
cd ../../.. # repo root

FILEPATH=./frost/tests/fixtures/test.bag

# if [ -f "$FILEPATH" ]; then
#     echo "$FILEPATH exists"
#     exit 0
# fi

source ./scripts/setup_py.sh
PYTHON=$(get_python)
setup_venv

$PYTHON ./frost/tests/scripts/gen.py --output "$FILEPATH" --count 100
