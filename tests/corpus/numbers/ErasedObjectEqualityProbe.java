class ErasedEqualsParent {
    int calls;
    public boolean equals(Object other) {
        calls++;
        return other == null || other instanceof String && "key".equals(other);
    }
}
class ErasedEqualsChild extends ErasedEqualsParent { }
class ErasedEqualsThrowing {
    public boolean equals(Object other) { throw new IllegalStateException("equals-called"); }
}
class ErasedEqualsReentrant {
    int calls;
    void changed() { calls++; }
    public boolean equals(Object other) { changed(); return other == this; }
}
class ErasedEqualsIdentity { int value; }
class ErasedEqualsGeneric<T> {
    int calls;
    public boolean equals(Object other) { calls++; return other == null; }
}
class ErasedObjectEqualityProbe {
    static int argumentCalls;
    static Object argument() { argumentCalls++; return "key"; }
    static boolean invoke(Object receiver, Object argument) { return receiver.equals(argument); }
    static int run() {
        ErasedEqualsChild child = new ErasedEqualsChild();
        Object mixed = child;
        if (!mixed.equals("key") || !invoke(mixed, null)) return 1;
        if (mixed.equals(1) || mixed.equals(mixed) || child.calls != 4) return 2;
        if ("key".equals(mixed) || Integer.valueOf(1).equals(mixed)) return 12;
        if (child.calls != 4) return 13;
        Object throwing = new ErasedEqualsThrowing();
        try { invoke(throwing, null); return 3; }
        catch (IllegalStateException error) {
            if (!"equals-called".equals(error.getMessage())) return 4;
        }
        ErasedEqualsReentrant reentrant = new ErasedEqualsReentrant();
        Object reentrantObject = reentrant;
        if (!invoke(reentrantObject, reentrantObject) || reentrant.calls != 1) return 5;
        if (invoke(reentrantObject, null) || reentrant.calls != 2) return 6;
        ErasedEqualsIdentity first = new ErasedEqualsIdentity();
        ErasedEqualsIdentity second = new ErasedEqualsIdentity();
        Object identity = first;
        Object sibling = second;
        if (!invoke(identity, identity) || invoke(identity, sibling) || invoke(identity, null)) return 7;
        if (invoke(identity, "key") || invoke(identity, 1)) return 8;
        ErasedEqualsGeneric<ErasedEqualsIdentity> generic = new ErasedEqualsGeneric<>();
        Object genericObject = generic;
        if (!invoke(genericObject, null) || invoke(genericObject, "key") || generic.calls != 2) return 9;
        Object absent = null;
        try { absent.equals(argument()); return 10; }
        catch (NullPointerException error) { if (argumentCalls != 1) return 11; }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
