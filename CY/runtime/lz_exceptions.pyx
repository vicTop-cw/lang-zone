# lz_exceptions — LZ 异常层次 (Cython 实现)
# 基类: LzException → LzRuntimeError / LzValueError / LzIOError

cdef class LzException(Exception):
    def __init__(self, message: str):
        super().__init__(message)
        self._message = message

    cpdef str get_message(self):
        return self._message

cdef class LzRuntimeError(LzException):
    pass

cdef class LzValueError(LzException):
    pass

cdef class LzIOError(LzException):
    pass

def _lz_panic(msg: str):
    raise LzRuntimeError(msg)
