from typing import Dict

known_build_tasks: Dict[str, type] = {}


def add_build_task(task_name: str, class_to_call: type):
    if task_name in known_build_tasks:
        raise KeyError(f"Multiple instances of task name '{task_name}'")
    known_build_tasks.setdefault(task_name, class_to_call)
