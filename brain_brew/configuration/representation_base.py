import inspect
import logging


class RepresentationBase:
    @classmethod
    def from_dict(cls, data: dict):
        expected_values = {
            k: v for k, v in data.items()
            if k in inspect.signature(cls).parameters
        }

        if len(expected_values) != len(data):
            logging.warning(f"Unexpected values found when creating '{cls.__name__}': "
                            f"{[k for k, v in data.items() if k not in list(expected_values.keys())]}"
                            "\n!!! Please report this error if it seems strange")

        return cls(**expected_values)

    def encode(self):
        return {key: value for key, value in self.__dict__.items() if self.encode_filter(key, value)}

    @classmethod
    def encode_filter(cls, key, value):
        if value is None:
            return False
        if not value:
            return False
        return True
