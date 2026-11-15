const std = @import("std");

fn fib(n: i64) i64 { if (n <= 1) return n; return fib(n - 1) + fib(n - 2); }

fn sieve(alloc: std.mem.Allocator, n: usize) !usize {
    var p = try alloc.alloc(bool, n + 1);
    defer alloc.free(p);
    @memset(p, true); p[0] = false; p[1] = false;
    var i: usize = 2;
    while (i * i <= n) : (i += 1) { if (p[i]) { var j = i * i; while (j <= n) : (j += i) p[j] = false; } }
    var c: usize = 0; for (p) |v| if (v) c += 1;
    return c;
}

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    const alloc = gpa.allocator();
    std.debug.print("fib(35) = {d}
", .{fib(35)});
    std.debug.print("sieve(500000) = {d} primes
", .{try sieve(alloc, 500000)});
}
