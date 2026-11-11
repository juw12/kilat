# BUG (safety): find() may find nothing, the result is used as if it were always a number
sizes = [6, 8, 10, 12]
hit = next((s for s in sizes if s > 20), None)
print(hit + 1)
