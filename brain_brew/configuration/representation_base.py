

class RepresentationBase:
    @classmethod
    def from_dict(cls, data: dict):
        return cls(**data)  # noqa

    def encode(self):
        return {key: value for key, value in self.__dict__.items() if self.encode_filter(key, value)}

    @classmethod
    def encode_filter(cls, key, value):
        if value is None:
            return False
        if not value:
            return False
        return True
