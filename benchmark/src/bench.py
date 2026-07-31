def fib(n):
    return n if n <= 1 else fib(n-1)+fib(n-2)

def sieve(n):
    p = [True]*(n+1); p[0]=p[1]=False
    for i in range(2, int(n**0.5)+1):
        if p[i]: p[i*i:n+1:i] = [False]*((n - i*i)//i + 1)
    return sum(p)

print(f"fib(35) = {fib(35)}")
print(f"sieve(500000) = {sieve(500000)} primes")
