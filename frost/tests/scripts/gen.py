import argparse
import os

import rosbag
import rospy
from std_msgs.msg import Time, String


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

            t_msg = Time()
            t_msg.data.secs = t.secs
            t_msg.data.nsecs = t.nsecs

            bag.write('/chatter', s_msg, t=t)
            bag.write('/time', t_msg, t=t)
    finally:
        bag.close()
    
    print("Wrote {}".format(bag_path))

if __name__ == "__main__":
    main()