#!/usr/bin/env bash
set -e

# Move to the script dir then back to root so this script can be ran from anywhere
script_dir="${0%/*}"
cd "${script_dir}"
cd ../../.. # repo root

FILEPATH=./frost/tests/fixtures/decompressed.bag
LZ4_FILEPATH=./frost/tests/fixtures/compressed_lz4.bag

LARGE_FILEPATH=./frost/tests/fixtures/test_large.bag

source ./scripts/setup_py.sh
PYTHON=$(get_python)
setup_venv

$PYTHON ./frost/tests/scripts/gen.py --output "$FILEPATH" --count 100 
$PYTHON ./frost/tests/scripts/gen.py --output "$LZ4_FILEPATH" --count 100 --compression lz4

# ~1.3GB bag takes a long time to generate and is not used for unit tests
# $PYTHON ./frost/tests/scripts/gen.py --output "$LARGE_FILEPATH" --count 10000000
