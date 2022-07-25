#!/usr/bin/env bash
set -xe

script_dir="${0%/*}"
cd "${script_dir}"
cd ../../../ # repo root

if [ -d "./venv" ]; then
    source ./venv/bin/activate
else
    python3 venv venv
    source ./venv/bin/activate
    pip install --extra-index-url https://rospypi.github.io/simple/ rospy
    pip install --extra-index-url https://rospypi.github.io/simple/ rosbag
    pip install --extra-index-url https://rospypi.github.io/simple/ tf2_ros
fi

FILEPATH=./examples/read_bag/fixures/test.bag

if [ ! -f "$FILEPATH" ]; then
    python3 ./examples/read_bag/scripts/gen.py --output "$FILEPATH" --count 100
fi
