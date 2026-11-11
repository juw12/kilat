# n-body simulation (5 bodies): float arithmetic and list indexing.
from math import sqrt

solar_mass = 39.47841760435743
days_per_year = 365.24


def energy(x, y, z, vx, vy, vz, mass):
    e = 0.0
    n = len(x)
    for i in range(n):
        e += 0.5 * mass[i] * (vx[i] * vx[i] + vy[i] * vy[i] + vz[i] * vz[i])
        for j in range(i + 1, n):
            dx = x[i] - x[j]
            dy = y[i] - y[j]
            dz = z[i] - z[j]
            e -= mass[i] * mass[j] / sqrt(dx * dx + dy * dy + dz * dz)
    return e


def advance(x, y, z, vx, vy, vz, mass, dt, steps):
    n = len(x)
    for step in range(steps):
        for i in range(n):
            for j in range(i + 1, n):
                dx = x[i] - x[j]
                dy = y[i] - y[j]
                dz = z[i] - z[j]
                d2 = dx * dx + dy * dy + dz * dz
                mag = dt / (d2 * sqrt(d2))
                mi = mass[i] * mag
                mj = mass[j] * mag
                vx[i] -= dx * mj
                vy[i] -= dy * mj
                vz[i] -= dz * mj
                vx[j] += dx * mi
                vy[j] += dy * mi
                vz[j] += dz * mi
        for i in range(n):
            x[i] += dt * vx[i]
            y[i] += dt * vy[i]
            z[i] += dt * vz[i]


x = [0.0, 4.84143144246472090, 8.34336671824457987, 12.8943695621391310, 15.3796971148509165]
y = [0.0, -1.16032004402742839, 4.12479856412430479, -15.1111514016986312, -25.9193146099879641]
z = [0.0, -0.103622044471123109, -0.403523417114321381, -0.223307578892655734, 0.179258772950371181]
vx = [0.0, 0.00166007664274403694, -0.00276742510726862411, 0.00296460137564761618, 0.00268067772490389322]
vy = [0.0, 0.00769901118419740425, 0.00499852801234917238, 0.00237847173959480950, 0.00162824170038242295]
vz = [0.0, -0.0000690460016972063023, 0.0000230417297573763929, -0.0000296589568540237556, -0.0000951592254519715870]
mass = [1.0, 0.000954791938424326609, 0.000285885980666130812, 0.0000436624404335156298, 0.0000515138902046611451]

px = 0.0
py = 0.0
pz = 0.0
for i in range(5):
    vx[i] = vx[i] * days_per_year
    vy[i] = vy[i] * days_per_year
    vz[i] = vz[i] * days_per_year
    mass[i] = mass[i] * solar_mass
    px += vx[i] * mass[i]
    py += vy[i] * mass[i]
    pz += vz[i] * mass[i]
vx[0] = -px / solar_mass
vy[0] = -py / solar_mass
vz[0] = -pz / solar_mass

before = energy(x, y, z, vx, vy, vz, mass)
print(f"{before:.9f}")
advance(x, y, z, vx, vy, vz, mass, 0.01, 100000)
after = energy(x, y, z, vx, vy, vz, mass)
print(f"{after:.9f}")
