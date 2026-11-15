# lz_concurrency — LZ 并发运行时 (Cython 声明)
# Future/spawn/go 原语

cdef class LzFuture:
    cdef object _result
    cdef bint _done
    cdef object _thread
    cpdef object await_result(self)
    cpdef bint is_done(self)
