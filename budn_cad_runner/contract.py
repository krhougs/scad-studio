import ast

REQUIRED_MODEL_DETAILS = (
    "purpose",
    "key_dimensions",
    "intended_use",
    "assumptions",
    "interaction_notes",
    "manufacturing_or_placement_constraints",
)


def analyze_source_contract(source: str) -> dict:
    try:
        tree = ast.parse(source)
    except SyntaxError as error:
        return {
            "has_model_description": False,
            "syntax_error": f"{error.msg} at line {error.lineno}",
        }

    description = top_level_assignment(tree, "MODEL_DESCRIPTION")
    details = top_level_assignment(tree, "MODEL_DETAILS")
    return {
        "has_model_description": non_empty_string(description)
        and model_details_complete(details),
        "syntax_error": None,
    }


def top_level_assignment(tree: ast.Module, name: str):
    value = None
    for node in tree.body:
        if isinstance(node, ast.Assign):
            for target in node.targets:
                if name_target(target, name):
                    value = node.value
        elif (
            isinstance(node, ast.AnnAssign)
            and name_target(node.target, name)
            and node.value is not None
        ):
            value = node.value
    return value


def name_target(node, name: str) -> bool:
    return isinstance(node, ast.Name) and node.id == name


def non_empty_string(node) -> bool:
    return isinstance(node, ast.Constant) and isinstance(node.value, str) and bool(node.value.strip())


def model_details_complete(node) -> bool:
    if not isinstance(node, ast.Dict):
        return False
    fields = dict_fields(node)
    return all(detail_value_present(fields.get(field)) for field in REQUIRED_MODEL_DETAILS)


def dict_fields(node: ast.Dict) -> dict[str, ast.AST]:
    fields = {}
    for key, value in zip(node.keys, node.values):
        if isinstance(key, ast.Constant) and isinstance(key.value, str):
            fields[key.value] = value
    return fields


def detail_value_present(node) -> bool:
    if non_empty_string(node):
        return True
    if isinstance(node, (ast.Dict, ast.List)):
        return collection_has_content(node)
    return False


def collection_has_content(node) -> bool:
    for child in ast.walk(node):
        if child is node:
            continue
        if isinstance(child, ast.Constant):
            if isinstance(child.value, str):
                if child.value.strip():
                    return True
            elif child.value is not None:
                return True
    return False
