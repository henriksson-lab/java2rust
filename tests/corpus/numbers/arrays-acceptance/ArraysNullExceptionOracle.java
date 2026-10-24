import org.apache.commons.numbers.arrays.MultidimensionalCounter;
import org.apache.commons.numbers.arrays.Selection;
import org.apache.commons.numbers.arrays.SortInPlace;
import java.util.Arrays;

/** Independent direct production calls; the co-translated client is not invoked. */
public final class ArraysNullExceptionOracle {
    static String bits(double[] values) {
        StringBuilder out=new StringBuilder("[");
        for (int i=0;i<values.length;i++) { if(i!=0)out.append(", "); out.append(Long.toHexString(Double.doubleToRawLongBits(values[i]))); }
        return out.append(']').toString();
    }
    static void emit(int which) {
        double[] doubles={3,1,2}; int[] ints={3,1,2}; long[] longs={3,1,2};
        String status="OK\tfalse\tNULL";
        try {
            switch (which) {
                case 0: Selection.select((double[])null,0); break;
                case 1: Selection.select((double[])null,new int[]{0}); break;
                case 2: Selection.select((double[])null,0,1,0); break;
                case 3: Selection.select((double[])null,0,1,new int[]{0}); break;
                case 4: Selection.select((int[])null,0); break;
                case 5: Selection.select((int[])null,new int[]{0}); break;
                case 6: Selection.select((int[])null,0,1,0); break;
                case 7: Selection.select((int[])null,0,1,new int[]{0}); break;
                case 8: Selection.select((long[])null,0); break;
                case 9: Selection.select((long[])null,new int[]{0}); break;
                case 10: Selection.select((long[])null,0,1,0); break;
                case 11: Selection.select((long[])null,0,1,new int[]{0}); break;
                case 12: Selection.select(doubles,(int[])null); break;
                case 13: Selection.select(ints,(int[])null); break;
                case 14: Selection.select(longs,(int[])null); break;
                case 15: MultidimensionalCounter.of((int[])null); break;
                case 16: MultidimensionalCounter.of(2,3).toUni((int[])null); break;
                case 17: SortInPlace.ASCENDING.apply((double[])null,new double[0][]); break;
                case 18: SortInPlace.DESCENDING.apply(doubles,(double[][])null); break;
                case 19: SortInPlace.valueOf(null); break;
                default: throw new IllegalArgumentException("unknown null boundary");
            }

        } catch (RuntimeException error) {
            StringBuilder message=new StringBuilder();
            if(error.getMessage()==null)message.append("NULL");
            else for(int i=0;i<error.getMessage().length();i++)message.append(String.format("%04x",(int)error.getMessage().charAt(i)));
            status=error.getClass().getName()+"\t"+(error instanceof NullPointerException)+"\t"+message;
        }
        System.out.println(which+"\t"+status+"\t"+bits(doubles)+"\t"+Arrays.toString(ints)+"\t"+Arrays.toString(longs));
    }
    public static void main(String[] args) { for(int which=0;which<20;which++)emit(which); }
}
