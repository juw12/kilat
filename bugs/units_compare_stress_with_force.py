# BUG (units): compares a load (force) with a yield strength (pressure)
yield_strength_mpa: float = 250
load_kn: float = 12
if load_kn < yield_strength_mpa:
    print("safe")
else:
    print("overloaded")
