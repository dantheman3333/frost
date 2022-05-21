import argparse
import os

import rosbag
import rospy
from std_msgs.msg import Float64MultiArray, MultiArrayDimension, MultiArrayLayout, String


def main():
    parser = argparse.ArgumentParser("Generate a bag")
    parser.add_argument("-o", "--output", required=True)
    parser.add_argument("-c", "--count", required=True, type=int)
    args = parser.parse_args()

    bag_path = args.output
    count = args.count

    os.makedirs(os.path.dirname(bag_path), exist_ok=True)
    bag = rosbag.Bag(bag_path, 'w')

    try:
        for i in range(count):
            t = rospy.Time(secs=i, nsecs=1000 + i*1000)

            s_msg = String()
            s_msg.data = 'foo_{}'.format(i)

            array_msg = Float64MultiArray()
            array_msg.layout = MultiArrayLayout()
            array_msg.layout.data_offset = 0
            array_msg.layout.dim = [MultiArrayDimension('data',3,3)]
            array_msg.data = [0.0] * 3

            bag.write('/chatter', s_msg, t=t)
            bag.write('/array', array_msg, t=t)
    finally:
        bag.close()
    
    print("Wrote {}".format(bag_path))

if __name__ == "__main__":
    main()