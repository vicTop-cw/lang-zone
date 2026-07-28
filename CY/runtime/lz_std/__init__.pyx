# lz_std — LZ 标准库运行时 (Cython 实现)

cdef class LzOption:
    def __cinit__(self, object value = None, bint is_some = False):
        self._value = value
        self._is_some = is_some

    cpdef bint is_some(self):
        return self._is_some

    cpdef bint is_none(self):
        return not self._is_some

    cpdef object unwrap(self):
        if not self._is_some:
            raise ValueError("unwrap on None")
        return self._value

    cpdef object unwrap_or(self, default):
        if self._is_some:
            return self._value
        return default

    def __repr__(self):
        if self._is_some:
            return f"Some({self._value!r})"
        return "None"

def Some(object value):
    return LzOption(value, True)

def None_():
    return LzOption(None, False)


cdef class LzResult:
    def __cinit__(self, object value = None, bint is_ok = True):
        self._value = value
        self._is_ok = is_ok

    cpdef bint is_ok(self):
        return self._is_ok

    cpdef bint is_err(self):
        return not self._is_ok

    cpdef object unwrap(self):
        if not self._is_ok:
            raise ValueError(f"unwrap on Err: {self._value!r}")
        return self._value

    cpdef object unwrap_err(self):
        if self._is_ok:
            raise ValueError(f"unwrap_err on Ok: {self._value!r}")
        return self._value

    def __repr__(self):
        if self._is_ok:
            return f"Ok({self._value!r})"
        return f"Err({self._value!r})"

def Ok(object value):
    return LzResult(value, True)

def Err(object value):
    return LzResult(value, False)


cdef class LzBox:
    def __cinit__(self, object value):
        self._value = value

    cpdef object get(self):
        return self._value

    cpdef void set(self, object val):
        self._value = val

    def __repr__(self):
        return f"Box({self._value!r})"

def Box(object value):
    return LzBox(value)


cdef class LzRc:
    def __cinit__(self, object value):
        self._value = value

    cpdef object get(self):
        return self._value

    def __repr__(self):
        return f"Rc({self._value!r})"

def Rc(object value):
    return LzRc(value)


cdef class LzArc:
    def __cinit__(self, object value):
        self._value = value

    cpdef object get(self):
        return self._value

    def __repr__(self):
        return f"Arc({self._value!r})"

def Arc(object value):
    return LzArc(value)
