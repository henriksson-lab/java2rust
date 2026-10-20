import java.util.function.*;

class ReferenceValueCounter {
    int value;
    ReferenceValueCounter(int value) { this.value = value; }
    int get() { return value; }
}

class ReferenceValueOwner {
    ReferenceValueCounter counter;
    IntSupplier bound() { return counter::get; }
    IntSupplier explicit() { return this.counter::get; }
    IntSupplier late() { return () -> counter.get(); }
    Supplier<IntSupplier> nested() { return () -> counter::get; }
    IntSupplier shadow(ReferenceValueCounter counter) { return counter::get; }
}

class BoundFieldReferenceValues {
    static int run() {
        ReferenceValueOwner owner = new ReferenceValueOwner();
        owner.counter = new ReferenceValueCounter(3);
        IntSupplier bound = owner.bound();
        IntSupplier explicit = owner.explicit();
        IntSupplier late = owner.late();
        Supplier<IntSupplier> nested = owner.nested();
        owner.counter = new ReferenceValueCounter(9);
        if (bound.getAsInt() != 3 || explicit.getAsInt() != 3 || late.getAsInt() != 9) return 1;
        IntSupplier evaluated = nested.get();
        owner.counter = new ReferenceValueCounter(11);
        if (evaluated.getAsInt() != 9 || late.getAsInt() != 11) return 2;
        IntSupplier shadow = owner.shadow(new ReferenceValueCounter(15));
        if (shadow.getAsInt() != 15) return 3;
        owner.counter = null;
        try { owner.bound(); return 4; }
        catch (NullPointerException expected) { }
        try { owner.explicit(); return 5; }
        catch (NullPointerException expected) { }
        try { late.getAsInt(); return 6; }
        catch (NullPointerException expected) { }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
