import java.util.function.IntSupplier;

interface CallbackConsumer {
    int consume(IntSupplier callback);
}

class InvokingConsumer implements CallbackConsumer {
    public int consume(IntSupplier callback) { return callback.getAsInt(); }
}

class SavingConsumer implements CallbackConsumer {
    private IntSupplier saved;
    public int consume(IntSupplier callback) { saved = callback; return 1; }
    int read() { return saved.getAsInt(); }
}

class VirtualCallbackContracts {
    static int forward(IntSupplier callback) {
        InvokingConsumer consumer = new InvokingConsumer();
        return consumer.consume(callback);
    }
    static int forwarded() { return forward(() -> 10); }
    static int direct() {
        InvokingConsumer consumer = new InvokingConsumer();
        return consumer.consume(() -> 7);
    }
    static int dispatched() {
        CallbackConsumer consumer = new InvokingConsumer();
        return consumer.consume(() -> 8);
    }
    static int retained() {
        SavingConsumer consumer = new SavingConsumer();
        consumer.consume(() -> 6);
        return consumer.read();
    }
    static int caught() {
        CallbackConsumer consumer = new InvokingConsumer();
        try {
            return consumer.consume(() -> { throw new IllegalArgumentException("failure"); });
        } catch (IllegalArgumentException error) { return 9; }
    }
}
