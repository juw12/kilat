# BUG (safety): no value is returned when the load is small
def grade(load_n: float) -> str:
    if load_n > 10000:
        return "heavy"


print(grade(2000))
