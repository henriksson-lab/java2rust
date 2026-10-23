interface EqualityNeutral<T> {
    T neutral();
    default boolean sameAsNeutral() { return this.equals(neutral()); }
}

class EqualityString implements EqualityNeutral<Object> {
    int calls;
    public Object neutral() { return "key"; }
    public boolean equals(Object other) {
        calls++;
        return other instanceof String && "key".equals(other);
    }
}

class EqualityInherited extends EqualityString { }

class EqualityNull implements EqualityNeutral<Object> {
    int calls;
    public Object neutral() { return null; }
    public boolean equals(Object other) { calls++; return other == null; }
}

class EqualityThrowing implements EqualityNeutral<Object> {
    public Object neutral() { return "key"; }
    public boolean equals(Object other) { throw new IllegalStateException("equals-called"); }
}

class EqualityIdentity implements EqualityNeutral<EqualityIdentity> {
    EqualityIdentity value;
    public EqualityIdentity neutral() { return value; }
}

class EqualityGeneric<T> implements EqualityNeutral<Object> {
    T value;
    int calls;
    EqualityGeneric(T value) { this.value = value; }
    public Object neutral() { return value; }
    public boolean equals(Object other) { calls++; return other == value; }
}

class EqualityReentrant implements EqualityNeutral<EqualityReentrant> {
    int calls;
    public EqualityReentrant neutral() { return this; }
    void changed() { calls++; }
    public boolean equals(Object other) { changed(); return other == this; }
}

class GenericObjectEqualityOracle {
    static int run() {
        EqualityString mixed = new EqualityString();
        if (!mixed.sameAsNeutral() || mixed.calls != 1) return 1;
        EqualityInherited inherited = new EqualityInherited();
        if (!inherited.sameAsNeutral() || inherited.calls != 1) return 2;
        EqualityNull nullable = new EqualityNull();
        if (!nullable.sameAsNeutral() || nullable.calls != 1) return 3;
        try { new EqualityThrowing().sameAsNeutral(); return 4; }
        catch (IllegalStateException error) {
            if (!"equals-called".equals(error.getMessage())) return 5;
        }
        EqualityIdentity identity = new EqualityIdentity();
        identity.value = identity;
        if (!identity.sameAsNeutral()) return 6;
        EqualityIdentity sibling = new EqualityIdentity();
        sibling.value = sibling;
        identity.value = sibling;
        if (identity.sameAsNeutral()) return 7;
        EqualityGeneric<EqualityIdentity> generic = new EqualityGeneric<>(identity);
        if (!generic.sameAsNeutral() || generic.calls != 1) return 8;
        generic.value = sibling;
        if (!generic.sameAsNeutral() || generic.calls != 2) return 9;
        EqualityReentrant reentrant = new EqualityReentrant();
        if (!reentrant.sameAsNeutral() || reentrant.calls != 1) return 10;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
