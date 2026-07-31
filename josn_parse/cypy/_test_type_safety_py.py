def test_type_mismatch():
    x: int = "hello"
    return x

def test_return_mismatch() -> int:
    return "not an int"

def test_param_type(a: int) -> int:
    return a

def test_call_wrong_type() -> int:
    return test_param_type("wrong")

def test_undefined_name() -> int:
    return undefined_variable

class Point:
    def __init__(self, x=0):
        self.x: int = x

def test_struct_field_type() -> int:
    p = Point(x="wrong")
    return p.x

if __name__ == "__main__":
    print("Python type errors are only caught at runtime:")
    try:
        test_type_mismatch()
    except Exception as e:
        print(f"  test_type_mismatch: {type(e).__name__}: {e}")
    try:
        test_return_mismatch()
    except Exception as e:
        print(f"  test_return_mismatch: {type(e).__name__}: {e}")
    try:
        test_call_wrong_type()
    except Exception as e:
        print(f"  test_call_wrong_type: {type(e).__name__}: {e}")
    try:
        test_undefined_name()
    except Exception as e:
        print(f"  test_undefined_name: {type(e).__name__}: {e}")
    try:
        test_struct_field_type()
    except Exception as e:
        print(f"  test_struct_field_type: {type(e).__name__}: {e}")