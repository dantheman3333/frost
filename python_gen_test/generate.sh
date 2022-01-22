#!/bin/bash
python3 -m venv venv
. ./venv/bin/activate
pip install --extra-index-url https://rospypi.github.io/simple/ rospy
pip install --extra-index-url https://rospypi.github.io/simple/ rosbag
pip install --extra-index-url https://rospypi.github.io/simple/ tf2_ros

python3 python_gen_test/gen.py