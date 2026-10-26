package nestedbuilders;
import java.util.function.IntConsumer;
import java.util.function.LongConsumer;
public class NestedBuilderProbe {
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
