# lz_std — LZ 标准库运行时 (Cython 声明)

# ── Option 类型 ──
cdef class LzOption:
    cdef object _value
    cdef bint _is_some
    cpdef bint is_some(self)
    cpdef bint is_none(self)
    cpdef object unwrap(self)
    cpdef object unwrap_or(self, default)

# ── Result 类型 ──
cdef class LzResult:
    cdef object _value
    cdef bint _is_ok
    cpdef bint is_ok(self)
    cpdef bint is_err(self)
    cpdef object unwrap(self)
    cpdef object unwrap_err(self)

# ── 指针类型 ──
cdef class LzBox:
    cdef object _value
    cpdef object get(self)
    cpdef void set(self, object val)

cdef class LzRc:
    cdef object _value
    cpdef object get(self)

cdef class LzArc:
    cdef object _value
    cpdef object get(self)
