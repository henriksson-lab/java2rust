interface Network<A, B> { int size(); }
class Forwarders {
    static <A, B> boolean add(Network<A, B> destination, Network<A, B> source) {
        if (destination == null) return false;
        return destination.size() == source.size();
    }
    static <A, B, G extends Network<A, B>> boolean forward(G destination, Network<A, B> source) {
        return add(destination, source);
    }
}
class NetworkHolder<A, B, G extends Network<A, B>> {
    G value;
    NetworkHolder(G value) { this.value = value; }
    boolean forward(Network<A, B> source) { return Forwarders.add(value, source); }
}
