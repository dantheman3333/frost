#!/usr/bin/env bash
set -e

# Move to the script dir then back to root so this script can be ran from anywhere
script_dir="${0%/*}"
cd "${script_dir}"
cd ../../.. # repo root

FILEPATH=./frost/tests/fixtures/decompressed.bag
FILEPATH_LARGE=./frost/tests/fixtures/decompressed_large.bag
LZ4_FILEPATH=./frost/tests/fixtures/compressed_lz4.bag

source ./scripts/setup_py.sh
PYTHON=$(get_python)
setup_venv

$PYTHON ./frost/tests/scripts/gen.py --output "$FILEPATH_LARGE" --count 10000000 

#$PYTHON ./frost/tests/scripts/gen.py --output "$FILEPATH" --count 100 
#$PYTHON ./frost/tests/scripts/gen.py --output "$LZ4_FILEPATH" --count 100 --compression lz4
