#!/usr/bin/env python3
"""
cython_build.py — 编译 .pyx → .c/.pyd/.so

用法:
    python cython_build.py <file.pyx> [output_dir] [--inplace]
    python cython_build.py --batch <dir>      # 批量编译目录下全部 .pyx

依赖: pip install cython
"""
import sys
import os
import glob

def build_one(pyx_file, output_dir=None, inplace=False):
    """编译单个 .pyx 文件"""
    from Cython.Build import cythonize
    from distutils.core import setup
    
    output_dir = output_dir or os.path.dirname(pyx_file)
    build_dir = output_dir if inplace else os.path.join(output_dir, "build")
    
    # cythonize 返回 Extension 列表
    extensions = cythonize(
        pyx_file,
        language_level="3",
        build_dir=build_dir,
    )
    
    if not extensions:
        print(f"cythonize: {pyx_file} → OK (no C output needed)")
        return True
    
    # 生成编译命令
    print(f"cythonize: {pyx_file} → .c OK")
    print(f"Extensions: {[e.name for e in extensions]}")
    print(f"To build .pyd/.so, run:")
    print(f"  python setup.py build_ext --inplace")
    print(f"Or use pyximport:")
    print(f"  python -c \"import pyximport; pyximport.install()\"")
    return True

def build_batch(directory, output_dir=None):
    """批量编译目录下全部 .pyx 文件"""
    pyx_files = glob.glob(os.path.join(directory, "**/*.pyx"), recursive=True)
    if not pyx_files:
        print(f"No .pyx files found in {directory}")
        return False
    print(f"Found {len(pyx_files)} .pyx files in {directory}")
    success = 0
    for pyx in pyx_files:
        try:
            if build_one(pyx, output_dir):
                success += 1
        except Exception as e:
            print(f"FAILED: {pyx}: {e}")
    print(f"\nBatch result: {success}/{len(pyx_files)} succeeded")
    return success == len(pyx_files)

if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser(description="Build .pyx files with Cython")
    parser.add_argument("pyx_file", nargs="?", help=".pyx file to compile")
    parser.add_argument("output_dir", nargs="?", help="Output directory")
    parser.add_argument("--inplace", action="store_true", help="Build in place")
    parser.add_argument("--batch", help="Batch build all .pyx in directory")
    args = parser.parse_args()
    
    if args.batch:
        success = build_batch(args.batch, args.output_dir)
        sys.exit(0 if success else 1)
    elif args.pyx_file:
        success = build_one(args.pyx_file, args.output_dir, args.inplace)
        sys.exit(0 if success else 1)
    else:
        parser.print_help()
        sys.exit(1)
