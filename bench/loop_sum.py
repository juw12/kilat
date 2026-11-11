# Integer arithmetic in a counted loop.
def run(n):
    total = 0
    for i in range(n):
        total += i % 7 * (i % 3)
    return total


print(run(10000000))
