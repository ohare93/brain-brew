import sys
import os
sys.path.append(os.path.abspath('../'))

from brain_brew.representation.build_config.top_level_builder import TopLevelBuilder

build: str = TopLevelBuilder.build_yamale()
filepath = "../brain_brew/schemas/recipe.yaml"

with open(filepath, 'w') as fp:
    fp.write(build)
    fp.close()
