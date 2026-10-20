import java.util.*;

class ContractItem {
    int value;
    ContractItem(int value) { this.value = value; }
}

class ContractPair<A, B> {
    A first;
    B second;
    ContractPair(A first, B second) { this.first = first; this.second = second; }
    static <A, B> ContractPair<A, B> of(A first, B second) {
        return new ContractPair<>(first, second);
    }
}

class ContractUnorderedPair<A, B> extends ContractPair<A, B> {
    ContractUnorderedPair(A first, B second) { super(first, second); }
}

interface ContractFactory<V, E> { Set<E> create(V vertex); }

class ContractMaker<V, E> implements ContractFactory<V, E> {
    Set<E> made;
    public Set<E> create(V vertex) { made = new LinkedHashSet<>(); return made; }
}

class ContractStore<V, E> {
    Map<ContractPair<V, V>, Set<E>> index;
    ContractFactory<V, E> factory;
    ContractStore(Map<ContractPair<V, V>, Set<E>> index, ContractFactory<V, E> factory) {
        this.index = index;
        this.factory = factory;
    }
    ContractPair<V, V> attach(V first, V second, E edge) {
        ContractPair<V, V> pair = new ContractPair<>(first, second);
        Set<E> edges = index.get(pair);
        if (edges == null) {
            edges = factory.create(first);
            index.put(pair, edges);
        }
        edges.add(edge);
        return pair;
    }
    Set<E> read(ContractPair<V, V> pair) { return index.get(pair); }
    Set<E> probe(V first, V second) { return index.get(ContractPair.of(first, second)); }
    Set<E> probeNull(V second) { return probe(null, second); }
    void clear() { index.clear(); }
}

class NestedMapContracts {
    static int run() {
        ContractItem item = new ContractItem(3);
        ContractMaker<ContractItem, ContractItem> maker = new ContractMaker<>();
        Map<ContractPair<ContractItem, ContractItem>, Set<ContractItem>> index = new LinkedHashMap<>();
        Set<ContractPair<ContractItem, ContractItem>> keys = index.keySet();
        ContractStore<ContractItem, ContractItem> store = new ContractStore<>(index, maker);
        ContractPair<ContractItem, ContractItem> pair = store.attach(null, item, item);
        ContractPair<ContractItem, ContractItem> alias = pair;
        ContractPair<ContractItem, ContractItem> other = new ContractUnorderedPair<>(item, item);
        if (other == pair || keys.size() != 1) return 5;
        if (pair != alias || pair.first != null || pair.second != item) return 1;
        Set<ContractItem> edges = store.read(pair);
        edges.add(null);
        if (!maker.made.contains(null) || !index.get(pair).contains(null)) return 2;
        store.clear();
        if (!index.isEmpty() || !keys.isEmpty() || edges.size() != 2) return 3;
        edges.remove(item);
        if (maker.made.size() != 1 || !maker.made.contains(null)) return 4;
        return 0;
    }
}
