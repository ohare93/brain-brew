from dataclasses import dataclass
from typing import Union, List, Set

from brain_brew.file_manager import FileManager
from brain_brew.representation.build_config.representation_base import RepresentationBase
from brain_brew.representation.generic.media_file import MediaFile
from brain_brew.representation.json.crowd_anki_export import CrowdAnkiExport
from brain_brew.representation.yaml.note_model_repr import NoteModel
from brain_brew.representation.yaml.note_repr import Notes


@dataclass
class MediaToFromCrowdAnki:
    @dataclass
    class Representation(RepresentationBase):
        from_notes: bool
        from_note_models: bool

    @classmethod
    def from_repr(cls, data: Union[Representation, dict, bool]):
        rep: cls.Representation
        if isinstance(data, cls.Representation):
            rep = data
        elif isinstance(data, dict):
            rep = cls.Representation.from_dict(data)
        else:
            rep = cls.Representation(from_notes=data, from_note_models=data)  # Support single boolean for default

        return cls(
            from_notes=rep.from_notes,
            from_note_models=rep.from_note_models
        )

    from_notes: bool
    from_note_models: bool

    file_manager: FileManager = None

    def __post_init__(self):
        self.file_manager = FileManager.get_instance()

    def move_to_crowd_anki(self, notes: Notes, note_models: List[NoteModel], ca_export: CrowdAnkiExport) -> Set[str]:
        def move_media(media_files):
            for file in media_files:
                filename = file.filename
                if filename in ca_export.known_media:
                    ca_export.known_media[filename].set_override(file.source_loc)
                else:
                    ca_export.known_media.setdefault(
                        filename, MediaFile(ca_export.media_loc + filename,
                                            filename, MediaFile.ManagementType.TO_BE_CLONED, file.source_loc)
                    )

        all_media: Set[str] = set()

        if self.from_notes:
            notes_media = notes.get_all_media_references()
            move_media(notes_media)
            all_media = all_media.union(notes_media)

        if self.from_note_models:
            for model in note_models:
                model_media = model.get_all_media_references()
                move_media(model_media)
                all_media = all_media.union(model_media)

        return all_media

    def move_to_deck_parts(self, notes: Notes, note_models: List[NoteModel]):
        def move_media(media_files):
            for file in media_files:
                filename = file.filename
                dp_media_file = self.file_manager.media_file_if_exists(filename)
                if dp_media_file:
                    dp_media_file.set_override(file.source_loc)
                else:
                    self.file_manager.new_media_file(filename, file.source_loc)

        if self.from_notes:
            move_media(notes.get_all_media_references())

        if self.from_note_models:
            for model in note_models:
                move_media(model.get_all_media_references())
