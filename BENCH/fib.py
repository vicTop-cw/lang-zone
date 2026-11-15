# 性能测试 — Python 原生版本（等价于 fib.lz）
# 运行: python fib.py
def fib(n):
    if n <= 1:
        return n
    return fib(n - 1) + fib(n - 2)

n = 35
print(fib(n))
