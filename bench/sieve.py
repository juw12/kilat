# Sieve of Eratosthenes: list of bool, nested while loops.
def sieve(n):
    flags = [True] * (n + 1)
    flags[0] = False
    flags[1] = False
    i = 2
    while i * i <= n:
        if flags[i]:
            j = i * i
            while j <= n:
                flags[j] = False
                j += i
        i += 1
    primes = 0
    for flag in flags:
        if flag:
            primes += 1
    return primes


print(sieve(2000000))
