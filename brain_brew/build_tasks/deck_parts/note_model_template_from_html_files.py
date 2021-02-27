from dataclasses import dataclass, field
from typing import Optional

from brain_brew.commands.run_recipe.build_task import BuildPartTask
from brain_brew.configuration.part_holder import PartHolder
from brain_brew.configuration.representation_base import RepresentationBase
from brain_brew.representation.generic.html_file import HTMLFile
from brain_brew.representation.yaml.note_model_template import Template

html_separator = '\n\n--\n\n'


@dataclass
class TemplateFromHTML(BuildPartTask):
    @classmethod
    def task_name(cls) -> str:
        return r'note_model_template_from_html'

    @classmethod
    def task_regex(cls) -> str:
        return r'note_model_template[s]?_from_html'

    @classmethod
    def yamale_schema(cls) -> str:
        return f"""\
            part_id: str()
            html_file: str()
            template_name: str(required=False)
            browser_html_file: str(required=False)
            deck_override_id: int(required=False)
            save_to_file: str(required=False)
        """

    @dataclass
    class Representation(RepresentationBase):
        part_id: str
        html_file: str
        template_name: Optional[str] = field(default=None)
        browser_html_file: Optional[str] = field(default=None)
        deck_override_id: Optional[int] = field(default=None)
        save_to_file: Optional[str] = field(default=None)

    @classmethod
    def from_repr(cls, data: dict):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
            part_id=rep.part_id,
            template_name=rep.template_name or rep.part_id,
            html_file=HTMLFile.create_or_get(rep.html_file),
            browser_html_file=HTMLFile.create_or_get(rep.browser_html_file) if rep.browser_html_file else None,
            deck_override_id=rep.deck_override_id,
            save_to_file=rep.save_to_file
        )

    part_id: str
    template_name: str
    html_file: HTMLFile
    browser_html_file: Optional[HTMLFile]
    deck_override_id: Optional[int]
    save_to_file: Optional[str]

    def execute(self):
        main_data = self.html_file.get_data(deep_copy=True)
        browser_data = self.browser_html_file.get_data(deep_copy=True) if self.browser_html_file else None

        if html_separator not in main_data:
            raise ValueError(f"Cannot find separator {html_separator} in html file '{self.html_file.file_location}'")

        front, back = tuple(main_data.split(html_separator, maxsplit=1))

        if browser_data:
            if html_separator not in browser_data:
                raise ValueError(f"Cannot find separator {html_separator} in html file '{self.browser_html_file.file_location}'")

            browser_front, browser_back = tuple(browser_data.split(html_separator, maxsplit=1))
        else:
            browser_front = browser_back = ""

        template = Template(
            name=self.template_name,
            question_format=front,
            answer_format=back,
            question_format_in_browser=browser_front,
            answer_format_in_browser=browser_back,
            deck_override_id=self.deck_override_id
        )

        PartHolder.override_or_create(self.part_id, self.save_to_file, template)
