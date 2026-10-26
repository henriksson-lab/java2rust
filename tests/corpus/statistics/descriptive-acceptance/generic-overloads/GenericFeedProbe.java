package genericoverloads;
import java.util.function.DoubleConsumer;
import java.util.function.IntConsumer;
import java.util.function.LongConsumer;
interface IntState extends IntConsumer { int value(); }
class IntCell implements IntState {
    int sum; int calls;
    public int value(){ return sum; }
    public void accept(int x){ calls++; if(x<0) throw new IllegalArgumentException("negative"); sum+=x; }
}
class DoubleCell implements DoubleConsumer { double sum; public void accept(double x){sum+=x;} }
class LongCell implements LongConsumer { long sum; public void accept(long x){sum+=x;} }
public class GenericFeedProbe {
    static int[] kept;
    public static int run(){
        IntCell cell=new IntCell(); IntState view=cell;
        kept=new int[]{2,3,4};
        if(Feed.add(cell,kept)!=cell || cell.sum!=9) return 1;
        if(Feed.add(view,kept,1,3)!=view || cell.sum!=16) return 2;
        DoubleCell d=new DoubleCell();
        if(Feed.add(d,kept)!=d || d.sum!=9) return 3;
        if(Feed.add(d,new long[]{3000000000L,2})!=d || d.sum!=3000000011.0) return 4;
        if(Feed.add(d,new double[]{0.5,1.25})!=d || d.sum!=3000000012.75) return 5;
        LongCell l=new LongCell();
        if(Feed.add(l,new long[]{3000000000L,2})!=l || l.sum!=3000000002L) return 6;
        try { Feed.add(view,new int[]{1,-1,100}); return 7; }
        catch(IllegalArgumentException e){}
        if(cell.sum!=17 || cell.calls!=7) return 8;
        try { Feed.add(view,kept,2,1); return 9; }
        catch(IndexOutOfBoundsException e){}
        if(cell.sum!=17 || cell.calls!=7) return 10;
        if(Feed.add((IntConsumer)null,new int[0])!=null) return 11;
        try { Feed.add((IntConsumer)null,new int[]{1}); return 12; }
        catch(NullPointerException e){}
        return 0;
    }
    public static void main(String[] args){System.out.println(run());}
}
