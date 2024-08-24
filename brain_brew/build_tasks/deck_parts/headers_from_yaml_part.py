from dataclasses import dataclass, field
from typing import Union, Optional

from brain_brew.build_tasks.overrides.headers_override import HeadersOverride
from brain_brew.commands.run_recipe.build_task import BuildPartTask
from brain_brew.configuration.part_holder import PartHolder
from brain_brew.configuration.representation_base import RepresentationBase
from brain_brew.representation.yaml.headers import Headers


@dataclass
class HeadersFromYamlPart(BuildPartTask):
    @classmethod
    def yamale_schema(cls) -> str:
        return f'''\
            part_id: str()
            file: str()
            override: include('{HeadersOverride.task_name()}', required=False)
        '''

    @classmethod
    def yamale_dependencies(cls) -> set:
        return {HeadersOverride}

    @classmethod
    def task_name(cls) -> str:
        return r'headers_from_yaml_part'

    @classmethod
    def task_regex(cls) -> str:
        return r'headers?_from_yaml_part'

    @dataclass
    class Representation(RepresentationBase):
        part_id: str
        file: str
        override: Optional[dict] = field(default_factory=lambda: None)

        def encode(self):
            d = {
                "part_id": self.part_id,
                "file": self.file
            }
            if self.override:
                d.setdefault("override", self.override)
            return d

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
            rep=rep,
            headers=PartHolder.override_or_create(
                part_id=rep.part_id,
                save_to_file=None,
                part=Headers.from_yaml_file(rep.file)
            ).part,
            override=HeadersOverride.from_repr(rep.override) if rep.override else None
        )

    rep: Representation
    headers: Headers
    override: Optional[HeadersOverride]

    def execute(self):
        if self.override:
            self.headers = self.override.override(self.headers)
