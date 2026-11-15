import statistics, subprocess, time
def t(exe, n=9):
    xs = []
    for _ in range(n):
        s = time.perf_counter()
        subprocess.run([exe], stdout=subprocess.DEVNULL, check=True)
        xs.append((time.perf_counter() - s) * 1000)
    return statistics.median(xs)
new = t("out/str_new.exe"); old = t("out/str_old.exe")
print(f"push_str(新)          : {new:.1f} ms")
print(f"to_string+add(旧)      : {old:.1f} ms")
print(f"=> 提速 {old/new:.2f}x（耗时 -{old-new:.1f} ms, -{old/new-1:.0%}）")
