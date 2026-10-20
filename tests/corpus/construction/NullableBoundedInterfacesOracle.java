class NullableBoundedInterfacesOracle {
    static class Value implements Network<String, Integer> {
        public int size() { return 3; }
    }
    public static void main(String[] args) {
        Value value = new Value();
        if (!Forwarders.<String, Integer, Value>forward(value, value)
            || Forwarders.<String, Integer, Value>forward(null, value)
            || !Forwarders.<String, Integer, Value>forward(value, value)) throw new AssertionError("nullable forwarding");
        NetworkHolder<String, Integer, Value> holder = new NetworkHolder<>(value);
        if (!holder.forward(value) || !holder.forward(value)) throw new AssertionError("field forwarding");
        System.out.println("true:false:true");
    }
}
