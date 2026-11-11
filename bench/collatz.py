# Longest Collatz chain below a limit: integer division and branches.
def longest(limit):
    best = 0
    best_start = 0
    for start in range(1, limit):
        x = start
        steps = 0
        while x != 1:
            if x % 2 == 0:
                x = x // 2
            else:
                x = 3 * x + 1
            steps += 1
        if steps > best:
            best = steps
            best_start = start
    return best_start, best


start, steps = longest(100000)
print(f"{start} {steps}")
