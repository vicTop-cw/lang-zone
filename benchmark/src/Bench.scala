object Bench {
    def fib(n: Long): Long = if (n <= 1) n else fib(n-1) + fib(n-2)
    def sieve(n: Int): Int = {
        val p = Array.fill(n+1)(true); p(0)=false; p(1)=false
        for (i <- 2 to math.sqrt(n).toInt if p(i)) { for (j <- i*i to n by i) p(j) = false }
        p.count(_ == true)
    }
    def main(a: Array[String]): Unit = {
        println(s"fib(35) = ${fib(35)}")
        println(s"sieve(500000) = ${sieve(500000)} primes")
    }
}
