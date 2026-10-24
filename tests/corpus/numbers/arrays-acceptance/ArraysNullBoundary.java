import org.apache.commons.numbers.arrays.MultidimensionalCounter;
import org.apache.commons.numbers.arrays.Selection;
import org.apache.commons.numbers.arrays.SortInPlace;

/** Co-translated caller makes root-array nullability explicit, without changing production sources. */
public final class ArraysNullBoundary {
    public static int check(int which) {
        double[] doubles = {3,1,2};
        int[] ints = {3,1,2};
        long[] longs = {3,1,2};
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
        } catch (NullPointerException expected) {
            // Failed validation must precede data mutation.
            return doubles[0] == 3 && ints[0] == 3 && longs[0] == 3 ? 0 : 2;
        }
        return 1;
    }
    public static void main(String[] args) {
        for (int which=0; which<20; which++) System.out.println(which+"\t"+check(which));
    }
}
