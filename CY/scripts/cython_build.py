import sys
import os
from Cython.Build import cythonize

# 用 Cython API 编译 .pyx → .pyd
pyx_file = sys.argv[1]
output_dir = sys.argv[2] if len(sys.argv) > 2 else os.path.dirname(pyx_file)

# cythonize 返回 Extension 列表
extensions = cythonize(
    pyx_file,
    language_level="3",
    build_dir=output_dir,
)

# 如果没有 Extension，pyx 已编译
if not extensions:
    print(f"cythonize: {pyx_file} → OK (no C output needed)")
    sys.exit(0)

# 生成编译命令
print(f"cythonize: {pyx_file} → .c OK")
print(f"Now compile .c with: python -c \"import pyximport; pyximport.install()\"")
print(f"Or build .pyd with: python setup.py build_ext --inplace")
sys.exit(0)
