from dataclasses import dataclass


from brain_brew.representation.build_config.build_task import TopLevelBuildTask
from brain_brew.utils import all_combos_prepend_append


@dataclass
class CrowdAnkiGenerate(TopLevelBuildTask):
    task_names = all_combos_prepend_append(["CrowdAnki", "CrowdAnki Export"], "Generate ", "s")

