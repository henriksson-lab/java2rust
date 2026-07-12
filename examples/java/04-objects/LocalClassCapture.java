package examples.objects;

public class LocalClassCapture {
    public int addLater(int base, int value) {
        final int offset = 3;

        class Adder {
            int apply(int input) {
                return base + offset + input;
            }
        }

        return new Adder().apply(value);
    }
}

