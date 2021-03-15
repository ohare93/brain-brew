import logging
import os
from typing import List

from brain_brew.representation.generic.html_file import HTMLFile
from brain_brew.representation.yaml.note_model import NoteModel, CSS_FILE, TEMPLATES
from brain_brew.representation.yaml.note_model_template import HTML_FILE as TEMPLATE_HTML_FILE, NAME as TEMPLATE_NAME, BROWSER_HTML_FILE as TEMPLATE_BROWSER_HTML_FILE
from brain_brew.representation.yaml.yaml_object import YamlObject
from brain_brew.utils import create_path_if_not_exists, clear_contents_of_folder


def save_note_model_to_location(
        model: NoteModel,
        folder: str,
        clear_folder: bool
) -> str:

    nm_folder = os.path.join(folder, model.name + '/')
    create_path_if_not_exists(nm_folder)

    if clear_folder:
        clear_contents_of_folder(nm_folder)

    model_encoded = model.encode_as_part_with_empty_file_references()

    model_encoded[CSS_FILE.name] = os.path.join(nm_folder, "style.css")
    HTMLFile.write_file(model_encoded[CSS_FILE.name], model.css)

    templates_dict = {t.name: t for t in model.templates}

    for template_data in model_encoded[TEMPLATES.name]:
        name = template_data[TEMPLATE_NAME.name]
        template = templates_dict[name]
        t_data, b_t_data = template.get_template_files_data()

        template_data[TEMPLATE_HTML_FILE.name] = os.path.join(nm_folder, HTMLFile.to_filename_html(name))
        HTMLFile.write_file(template_data[TEMPLATE_HTML_FILE.name], t_data)

        if TEMPLATE_BROWSER_HTML_FILE.name in template_data and b_t_data is not None:
            template_data[TEMPLATE_BROWSER_HTML_FILE.name] = os.path.join(nm_folder, HTMLFile.to_filename_html(name + "_browser"))
            HTMLFile.write_file(template_data[TEMPLATE_HTML_FILE.name], b_t_data)

    model_yaml_file_name = YamlObject.to_filename_yaml(os.path.join(nm_folder, model.name))
    model.dump_to_yaml(model_yaml_file_name, model_encoded)

    return model_yaml_file_name
