import abc


class Verifiable(abc.ABC):
    @abc.abstractmethod
    def verify_contents(self):
        pass
