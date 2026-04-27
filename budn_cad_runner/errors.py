class RunnerError(Exception):
    pass


class BuildError(Exception):
    def __init__(self, original: BaseException):
        self.original = original
        super().__init__(str(original))
