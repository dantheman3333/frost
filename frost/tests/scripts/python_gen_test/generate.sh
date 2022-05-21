#!/bin/bash
set -xe

# Move to the script dir then back to root so this script can be ran from anywhere
script_dir="${0%/*}"
cd "${script_dir}"
cd ../../../.. # repo root

if [ -d "./venv" ]; then
    source ./venv/bin/activate
else
    python3 venv venv
    source ./venv/bin/activate
    pip install --extra-index-url https://rospypi.github.io/simple/ rospy
    pip install --extra-index-url https://rospypi.github.io/simple/ rosbag
    pip install --extra-index-url https://rospypi.github.io/simple/ tf2_ros
fi

FILEPATH=./frost/tests/fixtures/test.bag

if [ ! -f "$FILEPATH" ]; then
    python3 ./frost/tests/scripts/python_gen_test/gen.py --output "$FILEPATH" --count 1000
fi