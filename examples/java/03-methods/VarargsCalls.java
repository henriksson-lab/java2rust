package examples.methods;

public class VarargsCalls {
    public int sum(String label, int... values) {
        int total = 0;
        for (int value : values) {
            total += value;
        }
        return total;
    }

    public int callZero() {
        return sum("zero");
    }

    public int callMany() {
        return sum("many", 1, 2, 3);
    }

    public int callArray(int[] values) {
        return sum("array", values);
    }
}

