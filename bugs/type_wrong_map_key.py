# BUG (types): the map is keyed by part name but is read with a number
stock: dict[str, int] = {"bolt": 120, "nut": 300}
print(stock[0])
