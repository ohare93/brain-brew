import logging
import os
import sys

import yamale
from yamale import YamaleError
from yamale.schema import Schema
from yamale.validators import DefaultValidators

validators = DefaultValidators.copy()


class YamlVerifier:
    __instance = None
    recipe_schema: Schema

    def __init__(self):
        if YamlVerifier.__instance is None:
            YamlVerifier.__instance = self
        else:
            raise Exception("Multiple YamlVerifiers created")

        path = os.path.join(os.path.dirname(__file__), "schemas/recipe.yaml")
        self.recipe_schema = yamale.make_schema(path, parser='ruamel', validators=validators)

    @staticmethod
    def get_instance() -> 'YamlVerifier':
        return YamlVerifier.__instance

    def verify_recipe(self, filename):
        data = yamale.make_data(filename)
        try:
            yamale.validate(self.recipe_schema, data)
        except YamaleError as e:
            print('Validation failed!\n')
            for result in e.results:
                print("Error validating data '%s' with '%s'\n\t" % (result.data, result.schema))
                for error in result.errors:
                    print('\t%s' % error)
            exit(1)
        logging.info(f"Builder file {filename} is ✔")

    def build_from_parts(self, validators, extras):
        for val, extras in cls.known_validators():
            final_result = final_result + dedent(val) + "\n"
            if extras is not None:
                final_extras = final_extras.union(extras)
