from setuptools import setup, Extension

setup(
    name='_bench_cypy',
    version='0.1.0',
    ext_modules=[
        Extension(
            '_bench_cypy',
            sources=['output\\_bench_cypy.pyx'],
        ),
    ],
)