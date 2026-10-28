import java.util.Iterator;
import java.util.Spliterator;
import java.util.Spliterators;
import java.util.NoSuchElementException;
import java.util.stream.Stream;
import java.util.stream.StreamSupport;

public class IteratorStreamProbe implements Iterator<String> {
    int reads;
    boolean closed;
    boolean fail;
    static IteratorStreamProbe current;
    static int effects;
    static int characteristicsOperand() { effects++; return 0; }
    static boolean parallelOperand() { effects++; return false; }
    public boolean hasNext() {
        if (fail && reads == 1) throw new IllegalArgumentException("read failure");
        return !closed && reads < 3;
    }
    public String next() {
        if (!hasNext()) throw new NoSuchElementException();
        reads++;
        return reads == 1 ? "first" : reads == 2 ? null : "last";
    }
    static Stream<String> escaped() {
        IteratorStreamProbe owner = new IteratorStreamProbe();
        Spliterator<String> cursor = Spliterators.spliteratorUnknownSize(owner, Spliterator.ORDERED);
        return StreamSupport.stream(cursor, false);
    }
    static boolean visit(String value) {
        current.closed = true;
        return "first".equals(value);
    }
    static boolean nullValue(String value) { return value == null; }
    static int run() {
        current = new IteratorStreamProbe();
        Spliterator<String> saved = Spliterators.spliteratorUnknownSize(current, Spliterator.ORDERED);
        Stream<String> stream = StreamSupport.stream(saved, false);
        if (current.reads != 0) return 1;
        if (!stream.anyMatch(IteratorStreamProbe::visit) || current.reads != 1) return 2;
        try { stream.count(); return 3; } catch (IllegalStateException expected) {}
        if (escaped().count() != 3) return 4;
        Stream<String> closed = escaped();
        closed.close();
        try { closed.count(); return 5; } catch (IllegalStateException expected) {}
        try { Spliterators.spliteratorUnknownSize((Iterator<String>) null, 0); return 6; }
        catch (NullPointerException expected) {}
        effects = 0;
        try { Spliterators.spliteratorUnknownSize((IteratorStreamProbe) null, characteristicsOperand()); return 7; }
        catch (NullPointerException expected) {}
        if (effects != 1) return 8;
        try { StreamSupport.stream((Spliterator<String>) null, parallelOperand()); return 9; }
        catch (NullPointerException expected) {}
        if (effects != 2) return 10;
        current = new IteratorStreamProbe();
        Iterator<String> alias = current;
        Spliterator<String> first = Spliterators.spliteratorUnknownSize(alias, Spliterator.ORDERED | Spliterator.SIZED | Spliterator.SUBSIZED);
        Spliterator<String> second = Spliterators.spliteratorUnknownSize(alias, Spliterator.ORDERED);
        if (first == second) return 11;
        if (first.estimateSize() != Long.MAX_VALUE || first.characteristics() != Spliterator.ORDERED) return 12;
        if (!"first".equals(alias.next())) return 13;
        if (StreamSupport.stream(first, false).count() != 2) return 14;
        if (StreamSupport.stream(second, false).count() != 0) return 15;
        if (!escaped().anyMatch(IteratorStreamProbe::nullValue)) return 16;
        if (!StreamSupport.stream(Spliterators.spliteratorUnknownSize(new IteratorStreamProbe(), 0), false)
                .anyMatch(IteratorStreamProbe::nullValue)) return 17;
        current = new IteratorStreamProbe();
        current.fail = true;
        Stream<String> failing = StreamSupport.stream(Spliterators.spliteratorUnknownSize(current, 0), false);
        if (current.reads != 0) return 18;
        try { failing.count(); return 19; } catch (IllegalArgumentException expected) {}
        if (current.reads != 1) return 20;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
