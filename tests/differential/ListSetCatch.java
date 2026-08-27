import java.util.ArrayList;
import java.util.List;

public class ListSetCatch {
    public static String run() {
        List<String> xs = new ArrayList<String>();
        xs.add("a");
        xs.add("b");
        String old = xs.set(1, "c");
        String high = "";
        try {
            xs.set(3, "x");
            high = "miss";
        } catch (IndexOutOfBoundsException e) {
            high = "high";
        }
        String negative = "";
        try {
            xs.set(-1, "x");
            negative = "miss";
        } catch (IndexOutOfBoundsException e) {
            negative = "negative";
        }
        return old + ":" + xs.get(0) + ":" + xs.get(1) + ":" + high + ":" + negative;
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
