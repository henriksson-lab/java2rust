import java.util.*;

interface CollectionProducer<T> { Set<T> values(); }

class StoredCollectionProducer<T> implements CollectionProducer<T> {
    Set<T> stored = new LinkedHashSet<>();
    int reads;
    public Set<T> values() { reads++; return stored; }
}

class NullableSourceCollectionResults {
    static int copySize(CollectionProducer<Integer> source) {
        return new ArrayList<>(source.values()).size();
    }
    static int run() {
        StoredCollectionProducer<Integer> source = new StoredCollectionProducer<>();
        source.stored.add(7);
        if (copySize(source) != 1 || source.reads != 1) return 1;
        source.stored = null;
        try { copySize(source); return 2; }
        catch (NullPointerException expected) { }
        return source.reads == 2 ? 0 : 3;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
