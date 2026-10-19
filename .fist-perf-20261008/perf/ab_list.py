import statistics, subprocess, time
def t(exe, n=7):
    xs = []
    for _ in range(n):
        s = time.perf_counter()
        subprocess.run([exe], stdout=subprocess.DEVNULL, check=True)
        xs.append((time.perf_counter() - s) * 1000)
    return statistics.median(xs)
old = t("out/list_old.exe"); new = t("out/list_new.exe")
print("clone+extend (旧): %8.1f ms" % old)
print("push        (新): %8.1f ms" % new)
print("=> %.2fx 提速（-%0.1f%%）" % (old / new, (1 - new / old) * 100))
