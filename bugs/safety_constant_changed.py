# BUG (safety): changes a value that was declared as fixed
from typing import Final

SAFETY_FACTOR: Final = 2.0
SAFETY_FACTOR = 1.0
print(SAFETY_FACTOR)
