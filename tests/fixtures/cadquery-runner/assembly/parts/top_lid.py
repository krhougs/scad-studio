import cadquery as cq

REFS = {"part": "top_lid", "features": {"top_surface": {"selector": 'faces(">Z")'}}}


def build(params=None):
    return cq.Workplane("XY").box(80, 60, 8)
