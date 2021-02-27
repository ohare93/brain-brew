import logging

from brain_brew.commands.argument_reader import BBArgumentReader
# sys.path.append(os.path.join(os.path.dirname(__file__), "dist"))
# sys.path.append(os.path.dirname(__file__))


def main():
    logging.basicConfig(level=logging.DEBUG)

    # Read in Arguments
    argument_reader = BBArgumentReader()
    command = argument_reader.get_parsed()

    command.execute()


if __name__ == "__main__":
    main()
