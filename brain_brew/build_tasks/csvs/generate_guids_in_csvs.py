import logging
from dataclasses import dataclass, field
from typing import List, Union, Optional

from brain_brew.commands.run_recipe.build_task import TopLevelBuildTask
from brain_brew.configuration.representation_base import RepresentationBase
from brain_brew.representation.generic.csv_file import CsvFile
from brain_brew.utils import single_item_to_list, generate_anki_guid


@dataclass
class GenerateGuidsInCsvs(TopLevelBuildTask):
    execute_immediately = True

    @classmethod
    def task_name(cls) -> str:
        return r'generate_guids_in_csvs'

    @classmethod
    def task_regex(cls) -> str:
        return r'generate_guids_in_csv[s]?'

    @classmethod
    def yamale_schema(cls) -> str:
        return f'''\
            source: any(str(), list(str()))
            columns: any(str(), list(str()))
            delimiter: str(required=False)
        '''

    @dataclass
    class Representation(RepresentationBase):
        source: Union[str, List[str]]
        columns: Union[str, List[str]]
        delimiter: Optional[str] = field(default=None)

        def encode_filter(self, key, value):
            if not super().encode_filter(key, value):
                return False
            if key == 'delimiter' and all(CsvFile.delimiter_matches_file_type(value, f) for f in single_item_to_list(self.source)):
                return False
            return True

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        csv_files = [CsvFile.create_or_get(csv) for csv in single_item_to_list(rep.source)]
        for c in csv_files:
            c.set_delimiter(rep.delimiter)
            c.read_file()
        return cls(
            rep=rep,
            sources=csv_files,
            columns=rep.columns
        )

    rep: Representation
    sources: List[CsvFile]
    columns: List[str]

    def execute(self):
        logging.info("Attempting to generate Guids")

        errors = []

        # Make sure the columns exist on all
        for source in self.sources:
            missing = [c for c in self.columns if c not in source.column_headers]
            if any(missing):
                errors.append(f"Csv '{source.file_location}' does not contain all the specified columns: {missing}")

        if errors:
            raise KeyError(errors)

        for source in self.sources:
            guids_generated = 0
            data = source.get_data()
            for row in data:
                for column_name in row.keys():
                    if column_name in self.columns and not row[column_name]:
                        row[column_name] = generate_anki_guid()
                        guids_generated += 1
            if guids_generated > 0:
                logging.info(f"Generated {guids_generated} guids in csv '{source.file_location}'")
                source.set_data(data)
                source.write_file()

        logging.info("Generate guids complete")
