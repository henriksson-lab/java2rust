import java.util.ArrayList;
import java.util.Collections;
import java.util.Comparator;

class NaturalRank implements Comparable<NaturalRank> {
    static int calls;
    int value;
    NaturalRank(int value) { this.value = value; }
    public int compareTo(NaturalRank other) {
        calls++;
        if (value < 0 || other.value < 0) throw new IllegalArgumentException("negative rank");
        return value - other.value;
    }
}
final class LowRank extends NaturalRank { LowRank(int value) { super(value); } }
final class HighRank extends NaturalRank { HighRank(int value) { super(value); } }

class NaturalComparableConsumers {
    static Comparator<NaturalRank> ascending() { return Comparator.naturalOrder(); }
    static Comparator<NaturalRank> descending() { return Collections.reverseOrder(); }
    static int noEager() {
        NaturalRank.calls = 0;
        ascending(); descending();
        return NaturalRank.calls;
    }
    static int compared() {
        NaturalRank left = new LowRank(9);
        NaturalRank right = new HighRank(2);
        return ascending().compare(left, right);
    }
    static int reversed() {
        NaturalRank left = new LowRank(9);
        NaturalRank right = new HighRank(2);
        return descending().compare(left, right);
    }
    static int caught() {
        NaturalRank left = new LowRank(-1);
        NaturalRank right = new HighRank(2);
        try { return ascending().compare(left, right); }
        catch (IllegalArgumentException error) { return 8; }
    }
    static int caughtReversed() {
        NaturalRank left = new LowRank(9);
        NaturalRank right = new HighRank(-1);
        try { return descending().compare(left, right); }
        catch (IllegalArgumentException error) { return 8; }
    }
    static int caughtSort() {
        ArrayList<NaturalRank> values = new ArrayList<>();
        values.add(new LowRank(-1)); values.add(new HighRank(2));
        try { values.sort(ascending()); return -1; }
        catch (IllegalArgumentException error) { return 8; }
    }
}
