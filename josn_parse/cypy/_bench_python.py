def fib_cycle(n):
    if n <= 1:
        return n
    a = 0
    b = 1
    i = 2
    while i <= n:
        temp = a + b
        a = b
        b = temp
        i = i + 1
    return b

def fib_recursive(n):
    if n <= 1:
        return n
    return fib_recursive(n - 1) + fib_recursive(n - 2)

def loop_sum(n):
    total = 0
    i = 0
    while i < n:
        total = total + i
        i = i + 1
    return total

def matrix_multiply(n):
    size = n
    total = 0
    i = 0
    while i < size:
        j = 0
        while j < size:
            k = 0
            while k < size:
                total = total + i * j * k
                k = k + 1
            j = j + 1
        i = i + 1
    return total

class Point:
    def __init__(self, x=0, y=0):
        self.x = x
        self.y = y

def struct_ops(n):
    total = 0
    i = 0
    while i < n:
        p = Point(i, i + 1)
        total = total + p.x + p.y
        i = i + 1
    return total

def run_all():
    r1 = fib_cycle(40)
    r2 = fib_recursive(30)
    r3 = loop_sum(10000000)
    r4 = matrix_multiply(100)
    r5 = struct_ops(1000000)
    return r1 + r2 + r3 + r4 + r5

if __name__ == "__main__":
    run_all()