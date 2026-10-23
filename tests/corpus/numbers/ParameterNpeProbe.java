package npeprobe;

import java.util.function.IntSupplier;

/** Calls only translated production-like operations; checks catchable diagnostics. */
public class ParameterNpeProbe {
    static int length(int[] values) { return values.length; }
    static int field(NpeData match) { return match.value; }
    static int invoke(NpeData data) { return data.read(); }
    static int search(String text) { return text.indexOf("x"); }
    static int searchFrom(String text) { return text.lastIndexOf('x', 0); }
    static int retainedLength(int[] values) {
        int[] alias = values;
        if (alias != values) return -1;
        return values.length;
    }
    static int secondLength(double ignored, int[] values) { return values.length; }
    static double pass(double value) { return value; }
    static double callAddRead(NpeData first, NpeData second) {
        return pass(first.weight + second.weight);
    }
    static int explicit(String text) {
        if (text == null) throw new NullPointerException("source-written message");
        return text.length();
    }
    public static int run() {
        try { length(null); return 1; }
        catch (NullPointerException e) {
            if (!e.getMessage().equals("Cannot read the array length because \"<parameter1>\" is null")) return 2;
        }
        try { field(null); return 3; }
        catch (NullPointerException e) {
            if (!e.getMessage().equals("Cannot read field \"value\" because \"<parameter1>\" is null")) return 4;
        }
        try { invoke(null); return 5; }
        catch (NullPointerException e) {
            if (!e.getMessage().equals("Cannot invoke \"npeprobe.NpeData.read()\" because \"<parameter1>\" is null")) return 6;
        }
        try { search(null); return 7; }
        catch (NullPointerException e) {
            if (!e.getMessage().equals("Cannot invoke \"String.indexOf(String)\" because \"<parameter1>\" is null")) return 8;
        }
        try { explicit(null); return 9; }
        catch (NullPointerException e) {
            if (!e.getMessage().equals("source-written message")) return 10;
        }
        if (field(new NpeData()) != 7 || invoke(new NpeData()) != 7) return 11;
        if (length(new int[]{1, 2}) != 2 || search("xyz") != 0) return 12;
        try { retainedLength(null); return 13; }
        catch (NullPointerException e) {
            if (!e.getMessage().equals("Cannot read the array length because \"<parameter1>\" is null")) return 14;
        }
        try { secondLength(0.0, null); return 15; }
        catch (NullPointerException e) {
            if (!e.getMessage().equals("Cannot read the array length because \"<parameter2>\" is null")) return 16;
        }
        try { searchFrom(null); return 17; }
        catch (NullPointerException e) {
            if (!e.getMessage().equals("Cannot invoke \"String.lastIndexOf(int, int)\" because \"<parameter1>\" is null")) return 18;
        }
        try { new NpeBox(null); return 19; }
        catch (NullPointerException e) {
            if (!e.getMessage().equals("Cannot invoke \"String.length()\" because \"<parameter1>\" is null")) return 20;
        }
        if (retainedLength(new int[]{1, 2}) != 2 || new NpeBox("xyz").size != 3) return 21;
        if (secondLength(0.0, new int[]{1}) != 1 || searchFrom("xyz") != 0) return 22;
        try { new NpeStagedOwner(null, new NpeData()); return 23; }
        catch (NullPointerException e) {
            if (!e.getMessage().equals("Cannot invoke \"npeprobe.NpeData.normalized()\" because \"<parameter1>\" is null")) return 24;
        }
        try { new NpeStagedOwner(new NpeData(), null); return 25; }
        catch (NullPointerException e) {
            if (!e.getMessage().equals("Cannot invoke \"npeprobe.NpeData.normalized()\" because \"<parameter2>\" is null")) return 26;
        }
        NpeStagedOwner owner = new NpeStagedOwner(new NpeData(), new NpeData());
        if (owner.getAsInt() != 14 || owner.getAsInt() != 14) return 27;
        try { callAddRead(new NpeData(), null); return 28; }
        catch (NullPointerException e) {
            if (!e.getMessage().equals("Cannot read field \"weight\" because \"<parameter2>\" is null")) return 29;
        }
        if (callAddRead(new NpeData(), new NpeData()) != 2.0) return 30;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}

class NpeData {
    int value = 7;
    double weight = 1.0;
    int read() { return value; }
    NpeData normalized() {
        if (value < 0) throw new IllegalStateException("negative");
        return this;
    }
}

class NpeBox {
    final int size;
    NpeBox(String text) { size = text.length(); }
}

class NpeStagedOwner implements IntSupplier {
    private final NpeData start;
    private final NpeData end;
    private final IntSupplier strategy;
    NpeStagedOwner(NpeData start, NpeData end) {
        this.start = start.normalized();
        NpeData ready = end.normalized();
        this.end = ready;
        strategy = new Step();
    }
    private final class Step implements IntSupplier {
        public int getAsInt() { return start.read() + end.read(); }
    }
    public int getAsInt() { return strategy.getAsInt(); }
}
