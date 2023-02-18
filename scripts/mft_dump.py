import os
import sys
from mft import PyMftParser, PyMftAttributeX10, PyMftAttributeX30


def main():
    mft_file = os.path.abspath(os.path.expanduser(sys.argv[1]))
    parser = PyMftParser(mft_file)
    for entry_or_error in parser.entries():
        if isinstance(entry_or_error, RuntimeError):
            continue

        print(f"Entry ID: {entry_or_error.entry_id}")

        for attribute_or_error in entry_or_error.attributes():
            if isinstance(attribute_or_error, RuntimeError):
                continue

            resident_content = attribute_or_error.attribute_content
            if resident_content:
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
