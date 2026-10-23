package appendcontracts;
import java.util.function.DoubleConsumer;

class HashValue implements DoubleConsumer {
    int calls;
    public void accept(double value) { calls += (int)value; }
    public int hashCode() { calls++; return 42; }
}
class NullText {
    public String toString() { return null; }
}
class BadText {
    public String toString() { throw new IllegalStateException("text failure"); }
}
class BadHash {
    public int hashCode() { throw new IllegalArgumentException("hash failure"); }
}
class ReentrantText {
    final StringBuilder target;
    ReentrantText(StringBuilder target) { this.target = target; }
    public String toString() { target.append("inner"); return "outer"; }
}

public class ObjectAppendProbe {
    static int trace;
    static StringBuilder builder(boolean absent) { trace = trace * 10 + 1; return absent ? null : new StringBuilder(); }
    static Object argument() { trace = trace * 10 + 2; return new ReentrantText(new StringBuilder()); }
    static String wrapped(BadText value) { return new StringBuilder().append(value).toString(); }
    static <T> String generic(T value) { return new StringBuilder().append(value).toString(); }

    static int run() {
        HashValue value = new HashValue();
        HashValue alias = value;
        StringBuilder builder = new StringBuilder();
        if (builder.append(alias) != builder) return 1;
        if (!builder.toString().equals("appendcontracts.HashValue@2a") || value.calls != 1) return 2;
        if (!new StringBuffer().append(alias).toString().equals("appendcontracts.HashValue@2a") || value.calls != 2) return 3;
        Object erased = value;
        if (!new StringBuilder().append(erased).toString().equals("appendcontracts.HashValue@2a") || value.calls != 3) return 4;
        if (!generic(alias).equals("appendcontracts.HashValue@2a") || value.calls != 4) return 5;

        if (!new StringBuilder().append((Object)null).toString().equals("null")) return 6;
        if (!new StringBuilder().append(new NullText()).toString().equals("null")) return 7;
        if (!new StringBuilder().append((CharSequence)"abc").append(new char[]{'x','y'}).append(7).toString().equals("abcxy7")) return 8;
        StringBuilder reentrant = new StringBuilder();
        if (!reentrant.append(new ReentrantText(reentrant)).toString().equals("innerouter")) return 9;

        BadText bad = new BadText();
        try { wrapped(bad); return 10; }
        catch (IllegalStateException error) { if (!error.getMessage().equals("text failure")) return 11; }
        try { new StringBuilder().append(new BadHash()); return 12; }
        catch (IllegalArgumentException error) { if (!error.getMessage().equals("hash failure")) return 13; }
        trace = 0;
        try { builder(true).append(argument()); return 14; }
        catch (NullPointerException expected) { if (trace != 12) return 15; }
        trace = 0;
        if (builder(false).append(argument()).length() == 0 || trace != 12) return 16;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
