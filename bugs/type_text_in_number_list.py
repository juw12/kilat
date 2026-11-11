# BUG (types): a text value slipped into a list of numbers
loads: list[float] = [120.0, 80.5, "95", 60.0]
print(sum(loads))
