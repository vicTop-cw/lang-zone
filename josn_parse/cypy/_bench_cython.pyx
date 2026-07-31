def fib_cycle(n: int) -> int:
    if n <= 1:
        return n
    cdef int a = 0
    cdef int b = 1
    cdef int i = 2
    cdef int temp
    while i <= n:
        temp = a + b
        a = b
        b = temp
        i = i + 1
    return b

def fib_recursive(n: int) -> int:
    if n <= 1:
        return n
    return fib_recursive(n - 1) + fib_recursive(n - 2)

def loop_sum(n: int) -> int:
    cdef int total = 0
    cdef int i = 0
    while i < n:
        total = total + i
        i = i + 1
    return total

def matrix_multiply(n: int) -> int:
    cdef int size = n
    cdef int total = 0
    cdef int i = 0
    cdef int j, k
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

cdef struct CPoint:
    int x
    int y

def struct_ops(n: int) -> int:
    cdef int total = 0
    cdef int i = 0
    cdef CPoint p
    while i < n:
        p.x = i
        p.y = i + 1
        total = total + p.x + p.y
        i = i + 1
    return total

def run_all() -> int:
    cdef int r1 = fib_cycle(40)
    cdef int r2 = fib_recursive(30)
    cdef int r3 = loop_sum(10000000)
    cdef int r4 = matrix_multiply(100)
    cdef int r5 = struct_ops(1000000)
    return r1 + r2 + r3 + r4 + r5