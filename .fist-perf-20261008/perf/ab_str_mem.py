import subprocess, sys
def peak(exe):
    best = []
    for _ in range(5):
        r = subprocess.run(["powershell","-NoProfile","-Command",
            f"(Get-Process -Id $pid).PeakWorkingSet64","-ErrorAction","SilentlyContinue"],capture_output=True)
    # 用Measure-Command + 进程对象不可靠，改用 .NET 事件：直接 spawn 并读WorkingSet
    import ctypes, ctypes.wintypes as wt
    for _ in range(5):
        p = subprocess.Popen([exe], stdout=subprocess.DEVNULL)
        h = ctypes.windll.kernel32.OpenProcess(0x1000, False, p.pid)
        ctypes.windll.psapi.GetProcessMemoryInfo
        class PMC(ctypes.Structure):
            _fields_=[("cb",wt.DWORD),("PageFaultCount",wt.DWORD),("PeakWorkingSetSize",ctypes.c_size_t),
                      ("WorkingSetSize",ctypes.c_size_t),("QuotaPeakPagedPoolUsage",ctypes.c_size_t),
                      ("QuotaPagedPoolUsage",ctypes.c_size_t),("QuotaPeakNonPagedPoolUsage",ctypes.c_size_t),
                      ("QuotaNonPagedPoolUsage",ctypes.c_size_t),("PagefileUsage",ctypes.c_size_t),
                      ("PeakPagefileUsage",ctypes.c_size_t)]
        m = PMC(); m.cb = ctypes.sizeof(PMC)
        ctypes.windll.psapi.GetProcessMemoryInfo(h, ctypes.byref(m), m.cb)
        ctypes.windll.kernel32.CloseHandle(h)
        p.wait(); best.append(m.PeakWorkingSetSize//1024)
    return max(best)
for name in ("str_old","str_new"):
    print(f"{name}: peak {peak('out/'+name+'.exe')} KB")
