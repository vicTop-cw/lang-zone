from setuptools import setup, Extension
from Cython.Build import cythonize
import os

os.chdir(os.path.dirname(os.path.abspath(__file__)))

extensions = [
    Extension("_bench_cypy", ["output/_bench_cypy.pyx"]),
    Extension("_bench_cython", ["_bench_cython.pyx"]),
]

setup(
    name="benchmarks",
    ext_modules=cythonize(extensions, compiler_directives={"language_level": "3"}),
    zip_safe=False,
)
