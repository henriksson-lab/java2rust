package idiom.gaps;

public class ArrayArgumentSliceView {
    String[] values;

    public int firstLength() {
        return lengthOfFirst(values);
    }

    public int localLength() {
        String[] local = new String[] {"alpha", "beta"};
        return lengthOfFirst(local);
    }

    private int lengthOfFirst(String[] items) {
        return items[0].length();
    }
}
