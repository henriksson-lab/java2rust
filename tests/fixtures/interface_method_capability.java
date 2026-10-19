interface ValuePredicate<T> {
    boolean test(T value);
}

class ReflexivePredicate<T> implements ValuePredicate<T> {
    public boolean test(T value) { return value.equals(value); }
}

class InterfaceMethodCapability {
    static <T> ValuePredicate<T> create() { return new ReflexivePredicate<T>(); }
}
