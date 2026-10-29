import os
import sys
from mft import PyMftParser, PyMftAttributeX10, PyMftAttributeX30


def main():
    mft_file = os.path.abspath(os.path.expanduser(sys.argv[1]))
    parser = PyMftParser(mft_file, skip_errors=True)
    for entry in parser.entries():
        print(f"Entry ID: {entry.entry_id}")

        for attribute in entry.attributes():
            resident_content = attribute.attribute_content
            if isinstance(resident_content, PyMftAttributeX10):
                print(f"Found an X10 attribute")
                print(f"Modified: {resident_content.modified}")
            if isinstance(resident_content, PyMftAttributeX30):
                print(f"Found an X30 attribute")
                print(f"Modified: {resident_content.modified}")
                print(f"Name: {resident_content.name}")

        print("--------------------------------")


if __name__ == "__main__":
    main()
