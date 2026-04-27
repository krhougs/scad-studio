import re

from .errors import RunnerError

SELECTOR_PATTERN = re.compile(r'^(faces|edges|vertices|wires|shells|solids)\("([^"]+)"\)$')
BLOCKED_CHARS = set("\"'`;()")


def parse_selector(selector: str) -> tuple[str, str]:
    match = SELECTOR_PATTERN.match(selector)
    if match is None:
        raise RunnerError(f"invalid selector: {selector}")
    method, expression = match.groups()
    validate_expression(expression)
    return method, expression


def validate_expression(expression: str):
    if not expression.strip():
        raise RunnerError("selector expression cannot be empty")
    if any(char in BLOCKED_CHARS for char in expression):
        raise RunnerError(f"selector expression contains blocked characters: {expression}")
    if "\0" in expression:
        raise RunnerError("selector expression contains NUL")


def evaluate_selector(cq_object, selector: str):
    method, expression = parse_selector(selector)
    selector_fn = getattr(cq_object, method, None)
    if selector_fn is None or not callable(selector_fn):
        raise RunnerError(f"CadQuery object does not support selector method: {method}")
    return selector_fn(expression)
