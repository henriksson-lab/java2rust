import java.util.ArrayList;
import java.util.List;

public class ListGetCatch {
    public static String run() {
        List<String> xs = new ArrayList<String>();
        xs.add("a");
        xs.add("b");
        String first = xs.get(0);
        String high = "";
        try {
            xs.get(2);
            high = "miss";
        } catch (IndexOutOfBoundsException e) {
            high = "high";
        }
        String negative = "";
        try {
            xs.get(-1);
            negative = "miss";
        } catch (IndexOutOfBoundsException e) {
            negative = "negative";
        }
        return first + ":" + high + ":" + negative;
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
