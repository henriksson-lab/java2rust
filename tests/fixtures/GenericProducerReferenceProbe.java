interface ProducedValue { int value(); void set(int value); }
class ProducedNumber implements ProducedValue {
    int current;
    ProducedNumber(int value) { current = value; }
    public int value() { return current; }
    public void set(int value) { current = value; }
}
interface ValueProducer<V> { V produce(); }
interface CheckedProducer<V> { V checked(); }
class CheckedNumberProducer implements CheckedProducer<ProducedNumber> {
    int seed;
    CheckedNumberProducer(int seed) { this.seed = seed; }
    public ProducedNumber checked() {
        if (seed < 0) throw new IllegalStateException("negative product");
        return new ProducedNumber(seed);
    }
}
class NumberProducer implements ValueProducer<ProducedNumber> {
    ProducedNumber last = new ProducedNumber(7);
    public ProducedNumber produce() {
        if (last != null && last.value() < 0) throw new IllegalStateException("negative product");
        return last;
    }
}
class GenericProducerReferenceProbe {
    static ProducedValue checkedLocal(CheckedProducer<? extends ProducedValue> source) {
        ProducedValue value = source.checked();
        return value;
    }
    static <P extends ProducedValue> ProducedValue adapt(ValueProducer<P> source) { return source.produce(); }
    static <Q extends ProducedValue> ProducedValue forward(ValueProducer<Q> source) { return adapt(source); }
    static ProducedValue wildcardReturn(ValueProducer<? extends ProducedValue> source) { return source.produce(); }
    static ProducedValue wildcardLocal(ValueProducer<? extends ProducedValue> source) {
        ProducedValue value = source.produce();
        return value;
    }
    static ProducedValue wildcardCollection(ValueProducer<? extends ProducedValue> source) {
        java.util.List<ProducedValue> values = new java.util.ArrayList<ProducedValue>();
        values.add(source.produce());
        values.add(null);
        if (values.get(1) != null) throw new IllegalStateException();
        return values.get(0);
    }
    static int run() {
        if (checkedLocal(new CheckedNumberProducer(17)).value() != 17) return 9;
        try { checkedLocal(new CheckedNumberProducer(-1)); return 10; }
        catch (IllegalStateException expected) { }
        NumberProducer producer = new NumberProducer();
        ProducedValue view = forward(producer);
        if (view != producer.last || view.value() != 7) return 1;
        view.set(9);
        if (producer.last.value() != 9) return 2;
        if (wildcardReturn(producer) != producer.last) return 3;
        if (wildcardLocal(producer) != producer.last) return 4;
        if (wildcardCollection(producer) != producer.last) return 5;
        producer.last.set(-1);
        try { wildcardLocal(producer); return 8; }
        catch (IllegalStateException expected) { }
        producer.last = null;
        if (forward(producer) != null || wildcardReturn(producer) != null) return 6;
        if (wildcardLocal(producer) != null || wildcardCollection(producer) != null) return 7;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
