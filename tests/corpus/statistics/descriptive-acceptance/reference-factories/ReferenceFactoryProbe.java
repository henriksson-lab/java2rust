package referencefactories;
import resultsum.Sum;
import java.util.function.Function;
public class ReferenceFactoryProbe {
    static Sum saved;
    public static int run() {
        inputsum.Sum input=new inputsum.Sum(3);
        Function<inputsum.Sum,Sum> factory=Sum::new;
        Sum first=factory.apply(input); saved=first;
        Sum second=factory.apply(input);
        if(first==second || first.source!=input || second.source!=input) return 1;
        input.add(4); if(saved.value()!=7 || first.value()!=7 || second.value()!=7) return 2;
        Sum.Builder ints=new Sum.Builder(); inputsum.Sum.Builder longs=new inputsum.Sum.Builder();
        int[] a=ints.build(); long[] b=longs.build();
        if(a.length!=0 || b.length!=0 || a!=ints.build() || b!=longs.build()) return 3;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
