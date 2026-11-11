# BUG (names): the field is `mass`, not `mas`
from dataclasses import dataclass


@dataclass
class Part:
    name: str
    mass: float


p = Part(name="bracket", mass=0.25)
print(p.mas)
