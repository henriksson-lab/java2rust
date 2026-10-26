package nestedbuilders;
import java.util.function.IntConsumer;
public final class Alpha {
    public final int value;
    Alpha(int value) { this.value=value; }
    public static final class Builder {
        int sum; int calls;
        public Builder add(int x) {
            calls++;
            if(x<0) throw new IllegalArgumentException("negative");
            sum+=x;
            return this;
        }
        public Alpha build() { return new Alpha(sum); }
        public IntConsumer consumer() { return x -> { add(x); }; }
        public int calls() { return calls; }
    }
    public static Builder builder() { return new Builder(); }
    public static Alpha of(int[] values) {
        final Builder b=new Builder();
        for(int x:values) b.add(x);
        return b.build();
    }
    public static <Builder> Builder keep(Builder value) { return value; }
}
