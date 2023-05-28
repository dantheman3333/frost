#!/usr/bin/env bash

# Utility functions for generating test fixtures

get_python() {
    if type python3 &>/dev/null; then
        echo "python3"
    elif type python &>/dev/null; then
        echo "python"
    else
        echo "no python interpreter found"
        exit 1
    fi
}

setup_venv() {
    # Sets up a venv if it does not exist. Should be called from repo root
    PYTHON=$(get_python)
    if [ -d "venv" ]; then
        source ./venv/bin/activate
    else
        $PYTHON -m venv venv
        source ./venv/bin/activate
        $PYTHON -m pip install --extra-index-url https://rospypi.github.io/simple/ rospy rosbag tf2_ros roslz4
    fi
}
