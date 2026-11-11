# Sorting one million pseudo-random integers (LCG data).
def make(n):
    xs = []
    seed = 42
    for i in range(n):
        seed = (seed * 1103515245 + 12345) % 2147483648
        xs.append(seed % 1000000)
    return xs


n = 1000000
xs = make(n)
ordered = sorted(xs)
print(f"{ordered[0]} {ordered[n // 2]} {ordered[n - 1]} {sum(ordered)}")
