package nestedbuilders;
import java.util.function.LongConsumer;
public final class Beta {
    public final long value;
    Beta(long value) { this.value=value; }
    public static final class Builder {
        long sum=10;
        public Builder add(long x) { sum+=x; return this; }
        public Beta build() { return new Beta(sum); }
        public LongConsumer consumer() { return x -> { add(x); }; }
    }
    public static Builder builder() { return new Builder(); }
    public static Beta of(long[] values) {
        final Builder b=new Builder();
        for(long x:values) b.add(x);
        return b.build();
    }
}
