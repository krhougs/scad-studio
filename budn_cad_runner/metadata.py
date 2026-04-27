def clean_float(value: float):
    rounded = round(float(value), 6)
    return 0.0 if rounded == -0.0 else rounded


def object_shape(cq_object):
    if hasattr(cq_object, "toCompound"):
        return cq_object.toCompound()
    if hasattr(cq_object, "val"):
        return cq_object.val()
    return cq_object


def bounding_box(shape) -> dict:
    box = shape.BoundingBox()
    return {
        "min": [clean_float(box.xmin), clean_float(box.ymin), clean_float(box.zmin)],
        "max": [clean_float(box.xmax), clean_float(box.ymax), clean_float(box.zmax)],
    }


def compute(cq_object) -> dict:
    shape = object_shape(cq_object)
    return {
        "bounding_box": bounding_box(shape),
        "volume": clean_float(shape.Volume()) if hasattr(shape, "Volume") else None,
        "surface_area": clean_float(shape.Area()) if hasattr(shape, "Area") else None,
    }
