package resultsum;
public class Sum {
    public final inputsum.Sum source;
    public Sum() { this.source=new inputsum.Sum(-10); }
    public Sum(inputsum.Sum source) { this.source=source; }
    public int value() { return source.value; }
    public static class Builder {
        private static final int[] EMPTY={};
        private int[] create(int[] input) { return input; }
        public int[] build() { return create(EMPTY); }
    }
}
