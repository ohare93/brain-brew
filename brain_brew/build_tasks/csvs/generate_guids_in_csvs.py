import logging
from dataclasses import dataclass
from typing import List, Union

from brain_brew.configuration.build_config.build_task import TopLevelBuildTask
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
        '''

    @dataclass
    class Representation(RepresentationBase):
        source: Union[str, List[str]]
        columns: Union[str, List[str]]

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
            sources=[CsvFile.create_or_get(csv) for csv in single_item_to_list(rep.source)],
            columns=rep.columns
        )

    sources: List[CsvFile]
    columns: List[str]

    def execute(self):
        logging.info("Attempting to generate Guids")

        errors = []

        # Make sure the columns exist on all
        for source in self.sources:
            if any([c not in source.column_headers for c in self.columns]):
                errors.append(f"Csv '{source.file_location}' does not contain all the specified columns.")

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
