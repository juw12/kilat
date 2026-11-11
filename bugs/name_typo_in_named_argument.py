# BUG (names): `nmae` is a typo for the field `name`
from dataclasses import dataclass


@dataclass
class Part:
    name: str
    qty: int


p = Part(nmae="bolt", qty=4)
print(p.name)
