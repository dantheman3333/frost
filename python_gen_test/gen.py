import os

import rosbag
import rospy
from std_msgs.msg import Int32, String

FIXTURE_PATH = './tests/fixtures/test.bag'
NUM_WRITE = 1000

def main():
    os.makedirs(os.path.dirname(FIXTURE_PATH), exist_ok=True)
    bag = rosbag.Bag(FIXTURE_PATH, 'w')

    try:
        for i in range(NUM_WRITE):
            t = rospy.Time(secs=i)

            s_msg = String()
            s_msg.data = 'foo_{}'.format(i)

            i_msg = Int32()
            i_msg.data = i

            bag.write('/chatter', s_msg, t=t)
            bag.write('/numbers', i_msg, t=t)
    finally:
        bag.close()
    
    print("Wrote {}".format(FIXTURE_PATH))

if __name__ == "__main__":
    main()