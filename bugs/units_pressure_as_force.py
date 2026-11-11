# BUG (units): passes a pressure where a force is expected
def bolt_stress(force_n: float, area_mm2: float) -> float:
    return force_n / area_mm2


preload_mpa: float = 180
print(f"{bolt_stress(preload_mpa, 58):.1f} MPa")
