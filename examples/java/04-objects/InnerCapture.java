package examples.objects;

public class InnerCapture {
    private final int base;

    public InnerCapture(int base) {
        this.base = base;
    }

    public class Adder {
        private final int offset;

        public Adder(int offset) {
            this.offset = offset;
        }

        public int apply(int value) {
            return base + offset + value;
        }
    }
}

