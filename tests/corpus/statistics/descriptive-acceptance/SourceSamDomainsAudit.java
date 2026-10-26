package sourcesamdomains;
public class SourceSamDomainsAudit {
    interface Range<A,B> { B apply(A values, int from, int to); }
    interface Reverse<X,Y> extends Range<Y,X> {}
    interface Fixed { int[] apply(int[] values); }
    interface Maker<A,B> { B make(A values); }
    static final class Held {
        int[] values;
        Held(int[] values) { this.values=values; }
    }
    private static final int[] GLOBAL={1,2};
    private static Range<int[],int[]> stored;
    static int[] keep(int[] values,int from,int to) {
        if(from<0 || to>values.length || from>=to) throw new IllegalArgumentException("range");
        values[from]+=to-from;
        return values;
    }
    static int[] touch(int[] values) { values[0]+=1; return values; }
    public static int run() {
        stored=SourceSamDomainsAudit::keep;
        int[] first=stored.apply(GLOBAL,0,2);
        if(first!=GLOBAL || GLOBAL[0]!=3) return 1;
        if(stored.apply(GLOBAL,1,2)!=first || first[1]!=3) return 2;
        int[] local={5,6};
        int[] other=stored.apply(local,0,1);
        if(other!=local || other==GLOBAL || local[0]!=6 || GLOBAL[0]!=3) return 3;
        Reverse<int[],int[]> reversed=SourceSamDomainsAudit::keep;
        if(reversed.apply(local,1,2)!=local || local[1]!=7) return 4;
        Fixed fixed=SourceSamDomainsAudit::touch;
        if(fixed.apply(GLOBAL)!=GLOBAL || first[0]!=4) return 5;
        Maker<int[],Held> maker=Held::new;
        Held held=maker.make(GLOBAL);
        if(held.values!=GLOBAL) return 6;
        held.values[1]=11;
        if(first[1]!=11) return 7;
        try { stored.apply(GLOBAL,1,1); return 8; }
        catch(IllegalArgumentException expected) { if(GLOBAL[0]!=4 || GLOBAL[1]!=11) return 9; }
        try { stored.apply(null,0,1); return 10; }
        catch(NullPointerException expected) { if(local[0]!=6 || local[1]!=7) return 11; }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
