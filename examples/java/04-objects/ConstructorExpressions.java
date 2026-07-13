package examples.objects;

public class ConstructorExpressions {
    static class Value {
        private final int count;

        Value() {
            this(0);
        }

        Value(int count) {
            this.count = count;
        }

        int count() {
            return count;
        }
    }

    public int defaultValue() {
        return new Value().count();
    }

    public int withArgument(int count) {
        return new Value(count).count();
    }
}
