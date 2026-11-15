# lz_concurrency — LZ 并发运行时 (Cython 实现)
# Future/spawn/go 原语

import threading

cdef class LzFuture:
    def __cinit__(self):
        self._result = None
        self._done = False
        self._thread = None

    cpdef object await_result(self):
        if self._thread is not None:
            self._thread.join()
        return self._result

    cpdef bint is_done(self):
        return self._done

    def _run(self, target, args):
        try:
            if args:
                self._result = target(*args)
            else:
                self._result = target()
        except Exception as e:
            self._result = e
        finally:
            self._done = True

def spawn_func(target, args=None):
    cdef LzFuture fut = LzFuture()
    if args is None:
        args = ()
    t = threading.Thread(target=fut._run, args=(target, args))
    fut._thread = t
    t.start()
    return fut
