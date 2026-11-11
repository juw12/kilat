# BUG (units): a plain 90 is passed as a speed (km/h was meant, m/s is assumed)
def braking_distance(speed_ms: float, decel: float) -> float:
    return speed_ms ** 2 / (2 * decel)


print(braking_distance(90, 7.5))
