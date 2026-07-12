package examples.expressions;

public class NumericPromotion {
    public long widen(byte b, short s, int i) {
        long total = b + s + i;
        return total * 2L;
    }

    public double mix(int count, float scale) {
        return count * scale + 0.5d;
    }
}

