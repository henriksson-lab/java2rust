package staticarrayidentity;
import java.util.function.Function;
public class StaticArrayIdentityAudit {
    private static final int[] EMPTY={};
    private static final int[] VALUES={1,2};
    private static int[] held;
    static void retain(int[] value) { held=value; }
    static int[] keep(int[] value) { return value; }
    static int[] read() { return EMPTY; }
    public static int run() {
        int[] first=keep(EMPTY); int[] second=read();
        if(first!=second || second!=EMPTY) return 1;
        int[] clone=EMPTY.clone(); if(clone==EMPTY || clone.length!=0) return 2;
        int[] values=keep(VALUES); values[0]=9;
        if(VALUES[0]!=9 || values!=VALUES) return 3;
        Function<int[],int[]> action=StaticArrayIdentityAudit::keep;
        int[] result=action.apply(VALUES);
        if(result!=VALUES) return 4;
        result[1]=7; if(VALUES[1]!=7) return 5;
        retain(EMPTY); if(held!=first) return 6;
        retain(EMPTY); if(held!=second) return 7;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
