package nullablenativesupplier;

import java.util.Optional;
import java.util.NoSuchElementException;
import java.util.function.Supplier;
import java.util.function.IntSupplier;

public final class NullableNativeSupplierProbe {
    static int entered;
    static int generated;
    static final class Term {
        final int number;
        Term(int number) { this.number=number; }
    }
    static int consume(Supplier<Term> supplier,boolean invoke) {
        entered++;
        return invoke ? supplier.get().number : 17;
    }
    static int consumeInt(IntSupplier supplier,boolean invoke) {
        entered++;
        return invoke ? supplier.getAsInt() : 19;
    }
    public static int run() {
        if (consume((Supplier<Term>)null,false)!=17 || entered!=1) return 1;
        try { consume((Supplier<Term>)null,true); return 2; }
        catch (NullPointerException expected) { if (entered!=2) return 3; }
        Supplier<Term> supplier=() -> { generated++; return new Term(7); };
        if (consume(supplier,true)!=7 || generated!=1 || entered!=3) return 4;
        if (consume(supplier,false)!=17 || generated!=1 || entered!=4) return 5;
        if (consumeInt((IntSupplier)null,false)!=19 || entered!=5) return 6;
        try { consumeInt((IntSupplier)null,true); return 7; }
        catch (NullPointerException expected) { if (entered!=6) return 8; }
        if (consumeInt(() -> 23,true)!=23 || entered!=7) return 9;
        Optional<Term> empty=Optional.empty();
        try { empty.get(); return 10; }
        catch (NoSuchElementException expected) { }
        Optional<Term> present=Optional.of(new Term(29));
        if (present.get().number!=29) return 11;
        return 0;
    }
    public static void main(String[] args) {
        int result=run();
        if (result!=0) throw new AssertionError("nullable Supplier: "+result);
        System.out.println(result);
    }
}
