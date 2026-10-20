class WriteCounter {
    int count;
    WriteCounter(int count) { this.count = count; }
}

class ClassBoundWrites {
    static <T extends WriteCounter> int set(T value, int count) {
        value.count = count;
        return value.count;
    }
    static <T extends WriteCounter> int expression(T value, int count) {
        return (value.count = count) + value.count;
    }
    static <T extends WriteCounter> int update(T value) {
        value.count = value.count + 1;
        return value.count;
    }
    static <T extends WriteCounter> int rhs(T value) {
        value.count = value.count + 1;
        return value.count + 1;
    }
    static <T extends WriteCounter> int reentrant(T value) {
        value.count = rhs(value);
        return value.count;
    }
    static int fail() { throw new IllegalArgumentException(); }
    static <T extends WriteCounter> int collision(T __java_bound_field_value, int count) {
        __java_bound_field_value.count = count;
        return __java_bound_field_value.count;
    }
    static <T extends WriteCounter> int throwBeforeNull(T value) {
        try {
            value.count = fail();
            return 1;
        } catch (IllegalArgumentException ex) {
            return 0;
        }
    }
    static <T extends WriteCounter> int nullAfterRhs(T value) {
        try {
            value.count = 42;
            return 1;
        } catch (NullPointerException ex) {
            return 0;
        }
    }
    static int run() {
        WriteCounter value = new WriteCounter(3);
        if (set(value, 7) != 7 || value.count != 7) return 1;
        if (expression(value, 11) != 22 || value.count != 11) return 2;
        if (update(value) != 12 || value.count != 12) return 3;
        if (reentrant(value) != 14 || value.count != 14) return 4;
        WriteCounter nil = null;
        if (throwBeforeNull(nil) != 0) return 5;
        if (nullAfterRhs(nil) != 0) return 6;
        if (collision(value, 21) != 21 || value.count != 21) return 7;
        return 0;
    }
}
