import cadquery as cq

REFS = {"component": "pcb_main", "features": {"board_body": {"tag": "board_body"}}}


def build(params=None):
    return cq.Workplane("XY").box(70, 50, 1.6).tag("board_body")
