import java.util.ArrayList;
import java.util.Collections;

public class CollectionsSortComparatorThrowCatch {
    static int compare(Integer a, Integer b) {
        throw new IllegalArgumentException("bad compare");
    }

    public static String run() {
        ArrayList<Integer> values = new ArrayList<>();
        values.add(2);
        values.add(1);
        try {
            Collections.sort(values, CollectionsSortComparatorThrowCatch::compare);
            return "miss";
        } catch (IllegalArgumentException ex) {
            return "caught";
        }
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
