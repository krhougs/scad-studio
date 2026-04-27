from .errors import BuildError


def run(build_fn, params):
    try:
        return build_fn(params)
    except Exception as error:
        raise BuildError(error) from error
