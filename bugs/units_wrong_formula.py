# BUG (units): stress is force divided by area, here it is multiplied
force_n: float = 5000
area_mm2: float = 40 * 10
stress_mpa = force_n * area_mm2
print(f"{stress_mpa:.2f} MPa")
