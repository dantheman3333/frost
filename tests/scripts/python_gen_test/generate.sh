#!/bin/bash
set -xe

script_dir="${0%/*}"
cd "${script_dir}"
cd ../../.. # repo root
pwd

python3 -m venv venv
source ./venv/bin/activate
pip install --extra-index-url https://rospypi.github.io/simple/ rospy
pip install --extra-index-url https://rospypi.github.io/simple/ rosbag
pip install --extra-index-url https://rospypi.github.io/simple/ tf2_ros

python3 ./tests/scripts/python_gen_test/gen.py --output ./tests/fixtures/test.bag --count 1000
#python3 ./tests/scripts/python_gen_test/gen.py --output ./tests/fixtures/test_large.bag --count 10000000