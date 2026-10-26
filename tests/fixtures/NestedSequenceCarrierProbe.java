import java.util.ArrayList;
import java.util.List;

class NestedSequenceHolder<O> {
    List<? extends List<? extends O>> sequences;
    int initialCount;
    NestedSequenceHolder(List<? extends List<? extends O>> sequences) {
        this.sequences = sequences;
        initialCount = NestedSequenceCarrierProbe.flat(sequences).size();
    }
    List<O> flattened() { return NestedSequenceCarrierProbe.flat(sequences); }
    int count() { return NestedSequenceCarrierProbe.count(sequences); }
}
class NestedSequenceCarrierProbe {
    static <T> int count(List<? extends List<? extends T>> sequences) {
        int result = 0;
        for (List<? extends T> sequence : sequences) result += sequence.size();
        return result;
    }
    static <T> List<T> flat(List<? extends List<? extends T>> sequences) {
        List<T> result = new ArrayList<>();
        for (List<? extends T> sequence : sequences) result.addAll(sequence);
        return result;
    }
    static <T> List<List<T>> wrap(List<T> sequence) {
        List<List<T>> result = new ArrayList<>();
        result.add(sequence);
        return result;
    }
    static int run() {
        List<String> sequence = new ArrayList<>();
        sequence.add("first");
        List<List<String>> sequences = wrap(sequence);
        NestedSequenceHolder<String> holder = new NestedSequenceHolder<>(sequences);
        sequence.add("second");
        if (count(sequences) != 2 || holder.count() != 2 || holder.initialCount != 1) return 1;
        List<String> flattened = holder.flattened();
        if (flattened.size() != 2 || !flattened.get(1).equals("second")) return 2;
        sequence.add("third");
        if (count(sequences) != 3 || holder.count() != 3 || flattened.size() != 2) return 3;
        if (sequences.get(0) != sequence) return 4;
        sequences.add(null);
        try { count(sequences); return 5; }
        catch (NullPointerException expected) { }
        try { holder.count(); return 6; }
        catch (NullPointerException expected) { }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
