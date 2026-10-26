package inputsum;
public class Sum {
    public int value;
    public Sum(int value) { this.value=value; }
    public void add(int value) { this.value+=value; }
    public static class Builder {
        private static final long[] EMPTY={};
        private long[] create(long[] input) { return input; }
        public long[] build() { return create(EMPTY); }
    }
}
