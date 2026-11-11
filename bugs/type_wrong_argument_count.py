# BUG (types): the function needs four arguments, three are given
def beam_deflection(load: float, length: float, modulus: float, inertia: float) -> float:
    return load * length ** 3 / (3 * modulus * inertia)


print(beam_deflection(500, 1.2, 200e9))
