# BUG (safety): the Welded case is not handled
def describe(kind: str, value: float | None = None) -> str:
    if kind == "bolted":
        return f"bolted with {value} bolts"
    elif kind == "glued":
        return "glued"


print(describe("welded", 4.0))
