import java.util.ArrayList;
import java.util.List;

public class ListRemoveIndexCatch {
    public static String run() {
        List<String> xs = new ArrayList<String>();
        xs.add("a");
        xs.add("b");
        xs.add("c");
        String removed = xs.remove(1);
        String high = "";
        try {
            xs.remove(3);
            high = "miss";
        } catch (IndexOutOfBoundsException e) {
            high = "high";
        }
        String negative = "";
        try {
            xs.remove(-1);
            negative = "miss";
        } catch (IndexOutOfBoundsException e) {
            negative = "negative";
        }
        return removed + ":" + xs.get(0) + ":" + xs.get(1) + ":" + high + ":" + negative;
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
