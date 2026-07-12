package examples.methods;

public class Varargs {
    public int sum(String label, int... values) {
        int total = 0;
        for (int value : values) {
            total += value;
        }
        return total;
    }
}

