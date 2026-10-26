package siblingbuilders;
import java.util.function.IntConsumer;
import java.util.function.LongConsumer;
final class Alpha {
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
final class Beta {
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
public class SiblingNestedBuilderAudit {
    public static int run() {
        if(Alpha.of(new int[]{2,3,4}).value!=9) return 1;
        if(Beta.of(new long[]{3000000000L,2}).value!=3000000012L) return 2;
        Alpha.Builder a=Alpha.builder();
        Beta.Builder b=Beta.builder();
        if(a.add(2).add(3)!=a || b.add(3000000000L)!=b) return 3;
        if(Alpha.keep(b)!=b) return 4;
        IntConsumer accept=a.consumer();
        LongConsumer wide=b.consumer();
        accept.accept(7); wide.accept(4);
        Alpha first=a.build(); Beta second=b.build();
        if(first.value!=12 || second.value!=3000000014L) return 5;
        try { accept.accept(-1); return 6; }
        catch(IllegalArgumentException expected) {}
        if(a.calls()!=4 || a.build().value!=12) return 7;
        accept.accept(1); wide.accept(2);
        if(a.build().value!=13 || b.build().value!=3000000016L) return 8;
        if(first.value!=12 || second.value!=3000000014L) return 9;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
