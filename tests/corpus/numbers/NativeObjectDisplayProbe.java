package objectcontracts;

import java.util.function.DoubleConsumer;

class PlainTag {
    int count;
    void bump() { count++; }
}

class NativeTag implements DoubleConsumer {
    int count;
    public void accept(double value) { count += (int)value; }
}

class HashedTag implements DoubleConsumer {
    int calls;
    public void accept(double value) { calls += (int)value; }
    public int hashCode() { calls++; return 0x12ab; }
}

class ThrowingHashTag implements DoubleConsumer {
    public void accept(double value) { if (value < 0) throw new IllegalArgumentException("negative"); }
    public int hashCode() { throw new IllegalStateException("hash failure"); }
}

class NamedTag implements DoubleConsumer {
    int count;
    public void accept(double value) { count++; }
    public String toString() { return "named-" + count; }
}

class InheritedNamedTag extends NamedTag {}

public class NativeObjectDisplayProbe {
    static int run() {
        PlainTag plain = new PlainTag();
        PlainTag alias = plain;
        String expected = plain.getClass().getName() + "@" + Integer.toHexString(plain.hashCode());
        if (!plain.toString().equals(expected) || !alias.toString().equals(expected)) return 1;
        alias.bump();
        if (!plain.toString().equals(expected) || plain.count != 1) return 2;

        NativeTag tag = new NativeTag();
        NativeTag tagAlias = tag;
        String text = tag.toString();
        if (!text.equals("objectcontracts.NativeTag@" + Integer.toHexString(tag.hashCode()))) return 3;
        tagAlias.accept(3);
        if (!tagAlias.toString().equals(text) || tag.count != 3 || tag != tagAlias) return 4;
        Object erased = tag;
        if (!erased.toString().equals(text)) return 5;
        if (!("value=" + tag).equals("value=" + text)) return 6;

        HashedTag hashed = new HashedTag();
        if (!hashed.toString().equals("objectcontracts.HashedTag@12ab")) return 7;
        if (hashed.calls != 1) return 8;
        HashedTag hashedAlias = hashed;
        if (!hashedAlias.toString().equals("objectcontracts.HashedTag@12ab") || hashed.calls != 2) return 9;

        ThrowingHashTag throwing = new ThrowingHashTag();
        ThrowingHashTag throwingAlias = throwing;
        try { throwingAlias.toString(); return 10; }
        catch (IllegalStateException error) { if (!error.getMessage().equals("hash failure")) return 11; }

        NamedTag named = new NamedTag();
        NamedTag namedAlias = named;
        namedAlias.accept(1);
        if (!named.toString().equals("named-1")) return 12;
        InheritedNamedTag child = new InheritedNamedTag();
        child.accept(1);
        if (!child.toString().equals("named-1")) return 13;

        NativeTag absent = null;
        if (!("value=" + absent).equals("value=null")) return 14;
        try { absent.toString(); return 15; }
        catch (NullPointerException expectedNull) { }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
