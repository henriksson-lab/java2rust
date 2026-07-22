import java.util.function.BiConsumer;

class FunctionalBiConsumerSlot {
    static void run(String name, int count) {
        apply(name, count, (s, n) -> {});
    }

    static void apply(String name, int count, BiConsumer<String, Integer> callback) {
        callback.accept(name, count);
    }
}
