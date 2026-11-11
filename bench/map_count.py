# Counting with a string-keyed dict.
def tally(n):
    names = [f"key{i}" for i in range(5000)]
    counts = {}
    for i in range(n):
        name = names[i * 7919 % 5000]
        counts[name] = counts.get(name, 0) + 1
    checksum = 0
    for name, hits in counts.items():
        checksum += hits * len(name)
    return checksum + len(counts)


print(tally(2000000))
