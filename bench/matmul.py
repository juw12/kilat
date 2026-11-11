# Matrix multiply on lists of lists of float.
def make(n, scale):
    rows = []
    for i in range(n):
        row = []
        for j in range(n):
            row.append(scale * (i - j) * (i + j))
        rows.append(row)
    return rows


def matmul(a, b, n):
    out = []
    for i in range(n):
        a_row = a[i]
        row = [0.0] * n
        for j in range(n):
            total = 0.0
            for k in range(n):
                total += a_row[k] * b[k][j]
            row[j] = total
        out.append(row)
    return out


n = 160
a = make(n, 1.0 / n / n)
b = make(n, 1.0 / n / n)
c = matmul(a, b, n)
middle = c[n // 2][n // 2]
corner = c[0][n - 1]
print(f"{middle:.9f} {corner:.9f}")
