import cadquery as cq

REFS = {"part": "bottom_case", "features": {"floor": {"selector": 'faces("<Z")'}}}


def build(params=None):
    return cq.Workplane("XY").box(82, 62, 10)
