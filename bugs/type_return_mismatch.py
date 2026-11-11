# BUG (types): the function promises a number but returns text on one path
def safety_factor(strength: float, stress: float) -> float:
    if stress <= 0:
        return "no load"
    return strength / stress


print(safety_factor(250e6, 0.0))
