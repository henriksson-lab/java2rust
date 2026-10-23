interface CheckedInterfaceEquals<T> {
    T argument();
    default boolean compare() { return this.equals(argument()); }
}

class CheckedInterfaceEqualsMutable implements CheckedInterfaceEquals<Object> {
    int calls;
    public Object argument() { return null; }
    public boolean equals(Object other) { calls++; return other == null; }
}

class CheckedInterfaceEqualsChild extends CheckedInterfaceEqualsMutable { }

class CheckedInterfaceEqualsFresh implements CheckedInterfaceEquals<CheckedInterfaceEqualsFresh> {
    int arguments;
    int equalities;
    public CheckedInterfaceEqualsFresh argument() { arguments++; return new CheckedInterfaceEqualsFresh(); }
    public boolean equals(Object other) { equalities++; return other instanceof CheckedInterfaceEqualsFresh; }
}

class CheckedInterfaceEqualsThrowing implements CheckedInterfaceEquals<Object> {
    public Object argument() { return "argument"; }
    public boolean equals(Object other) { throw new IllegalStateException("checked-equals"); }
}

class CheckedInterfaceEqualsProbe {
    static int run() {
        CheckedInterfaceEqualsMutable value = new CheckedInterfaceEqualsMutable();
        if (!value.compare() || value.calls != 1) return 1;
        CheckedInterfaceEquals<Object> erased = value;
        if (!erased.compare() || value.calls != 2) return 2;
        if (!erased.equals(null) || value.calls != 3) return 3;
        CheckedInterfaceEqualsChild child = new CheckedInterfaceEqualsChild();
        if (!child.compare() || child.calls != 1) return 4;
        CheckedInterfaceEquals<Object> inherited = child;
        if (!inherited.compare() || child.calls != 2) return 5;
        try { new CheckedInterfaceEqualsThrowing().compare(); return 6; }
        catch (IllegalStateException error) {
            if (!"checked-equals".equals(error.getMessage())) return 7;
        }
        CheckedInterfaceEquals<Object> throwing = new CheckedInterfaceEqualsThrowing();
        try { throwing.compare(); return 8; }
        catch (IllegalStateException error) {
            if (!"checked-equals".equals(error.getMessage())) return 9;
        }
        CheckedInterfaceEquals<Object> absent = null;
        try { absent.equals(null); return 10; }
        catch (NullPointerException error) { }
        CheckedInterfaceEqualsFresh fresh = new CheckedInterfaceEqualsFresh();
        if (!fresh.compare() || fresh.arguments != 1 || fresh.equalities != 1) return 11;
        CheckedInterfaceEquals<CheckedInterfaceEqualsFresh> mutableArgument = fresh;
        if (!mutableArgument.compare() || fresh.arguments != 2 || fresh.equalities != 2) return 12;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
