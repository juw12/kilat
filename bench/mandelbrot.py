# Mandelbrot set membership count: float arithmetic in tight loops.
def mandel(size, max_iter):
    inside = 0
    for row in range(size):
        ci = 2.0 * row / size - 1.0
        for col in range(size):
            cr = 3.0 * col / size - 2.0
            zr = 0.0
            zi = 0.0
            i = 0
            while i < max_iter and zr * zr + zi * zi <= 4.0:
                next_zr = zr * zr - zi * zi + cr
                zi = 2.0 * zr * zi + ci
                zr = next_zr
                i += 1
            if i == max_iter:
                inside += 1
    return inside


print(mandel(300, 200))
