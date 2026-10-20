interface Network<A, B> { int size(); }
class Forwarders {
    static <A, B> boolean add(Network<A, B> destination, Network<A, B> source) {
        if (destination == null) return false;
        return destination.size() == source.size();
    }
}
class MutableNetwork<A, B> implements Network<A, B> {
    int count;
    MutableNetwork(int count) { this.count = count; }
    public int size() { return count; }
    void increment() { count++; }
    Object duplicate() {
        MutableNetwork<A, B> copy = new MutableNetwork<A, B>(count);
        Forwarders.add(copy, this);
        return copy;
    }
    static boolean aliases() {
        MutableNetwork<String, Integer> value = new MutableNetwork<String, Integer>(3);
        MutableNetwork<String, Integer> alias = value;
        alias.increment();
        return value.size() == 4;
    }
}
