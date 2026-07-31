# lz_exceptions — LZ 异常层次 (Cython 声明)
# panic 与异常基类

cdef class LzException(Exception):
    cpdef str get_message(self)

cdef class LzRuntimeError(LzException):
    pass

cdef class LzValueError(LzException):
    pass

cdef class LzIOError(LzException):
    pass

def _lz_panic(msg: str):
    raise LzRuntimeError(msg)
