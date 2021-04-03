from dataclasses import dataclass, field
from typing import List, Union, Optional, Dict

from brain_brew.commands.run_recipe.build_task import TopLevelBuildTask
from brain_brew.configuration.part_holder import PartHolder
from brain_brew.configuration.representation_base import RepresentationBase
from brain_brew.representation.yaml.note_model import NoteModel
from brain_brew.transformers.save_note_model_to_location import save_note_model_to_location


@dataclass
class SaveNoteModelsToFolder(TopLevelBuildTask):
    @classmethod
    def task_name(cls) -> str:
        return r'save_note_models_to_folder'

    @classmethod
    def task_regex(cls) -> str:
        return r"save_note_models[s]?_to_folder"

    @classmethod
    def yamale_schema(cls) -> str:
        return f'''\
            parts: list(str())
            folder: str()
            clear_existing: bool(required=False)
        '''

    @dataclass
    class Representation(RepresentationBase):
        parts: List[str]
        folder: str
        clear_existing: Optional[bool] = field(default=None)

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
            rep=rep,
            parts=list(holder.part for holder in map(PartHolder.from_file_manager, rep.parts)),
            folder=rep.folder,
            clear_existing=rep.clear_existing or False,
        )

    rep: Representation
    parts: List[NoteModel]
    folder: str
    clear_existing: bool

    def execute(self) -> Dict[str, str]:
        model_yaml_files: Dict[str, str] = {}
        for model in self.parts:
            model_yaml_files.setdefault(
                model.name,
                save_note_model_to_location(model, self.folder, self.clear_existing)
            )
        return model_yaml_files
