import statistics, subprocess, time, sys
def t(exe, n=9):
    xs = []
    for _ in range(n):
        s = time.perf_counter()
        subprocess.run([exe], stdout=subprocess.DEVNULL, check=True)
        xs.append((time.perf_counter() - s) * 1000)
    return statistics.median(xs)
cap = t("out/alloc_cap.exe")
nocap = t("out/alloc_nocap.exe")
print(f"with_capacity median: {cap:.1f} ms")
print(f"Vec::new()    median: {nocap:.1f} ms")
print(f"delta: {cap-nocap:+.1f} ms ({(cap-nocap)/nocap:+.1%})  -> with_capacity is {abs(cap-nocap)/max(cap,nocap):.1%} faster than the slower side")
