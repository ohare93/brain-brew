from dataclasses import dataclass
from brain_brew.interfaces.command import Command


@dataclass
class InitRepo(Command):
    crowdanki_folder: str

    def execute(self):
        pass

        # Clone a setup repo structure
        #   Use UG setup

        # Populate the Recipe file with the
