import os

import rosbag
from std_msgs.msg import Int32, String

FIXTURE_PATH = './tests/fixtures/test.bag'

def main():
    os.makedirs(os.path.dirname(FIXTURE_PATH), exist_ok=True)
    bag = rosbag.Bag(FIXTURE_PATH, 'w')

    try:
        s = String()
        s.data = 'foo'

        i = Int32()
        i.data = 42

        bag.write('chatter', s)
        bag.write('numbers', i)
    finally:
        bag.close()
    
    print("Wrote {}".format(FIXTURE_PATH))

if __name__ == "__main__":
    main()