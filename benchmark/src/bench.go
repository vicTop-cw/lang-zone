package main
import "fmt"
func fib(n int64) int64 { if n <= 1 { return n }; return fib(n-1) + fib(n-2) }
func sieve(n int) int {
    p := make([]bool, n+1); for i := 0; i <= n; i++ { p[i] = true }; p[0], p[1] = false, false
    for i := 2; i*i <= n; i++ { if p[i] { for j := i * i; j <= n; j += i { p[j] = false } } }
    c := 0; for _, v := range p { if v { c++ } }; return c
}
func main() {
    fmt.Printf("fib(35) = %d\n", fib(35))
    fmt.Printf("sieve(500000) = %d primes\n", sieve(500000))
}
