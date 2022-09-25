#!/usr/bin/env bash
set -e

# Move to the script dir then back to root so this script can be ran from anywhere
script_dir="${0%/*}"
cd "${script_dir}"
cd ../../.. # repo root

FILEPATH=./frost/tests/fixtures/test.bag

if [ -f "$FILEPATH" ]; then
    echo "$FILEPATH exists"
    exit 0
fi

if [ -d "venv" ]; then
    source ./venv/bin/activate
else
    python3 -m venv venv
    source ./venv/bin/activate
    pip install --extra-index-url https://rospypi.github.io/simple/ rospy rosbag tf2_ros roslz4
fi

python3 ./frost/tests/scripts/gen.py --output "$FILEPATH" --count 100
