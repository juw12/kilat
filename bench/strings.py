# String building (list + join), split and scanning.
def build(n):
    parts = []
    for i in range(n):
        parts.append(f"item {i}")
    return ",".join(parts)


text = build(500000)
print(len(text))
total = 0
for word in text.split(","):
    if word.endswith("7"):
        total += len(word)
print(total)
print(text[0:24].upper())
