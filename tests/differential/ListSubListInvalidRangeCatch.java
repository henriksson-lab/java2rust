import java.util.ArrayList;
import java.util.List;

public class ListSubListInvalidRangeCatch {
    public static String run() {
        List<String> values = new ArrayList<String>();
        values.add("a");
        values.add("b");
        String reversed = "";
        try {
            values.subList(2, 1).size();
            reversed = "miss";
        } catch (IllegalArgumentException e) {
            reversed = "illegal";
        } catch (IndexOutOfBoundsException e) {
            reversed = "bounds";
        }

        String negative = "";
        try {
            values.subList(-1, -2).size();
            negative = "miss";
        } catch (IllegalArgumentException e) {
            negative = "illegal";
        } catch (IndexOutOfBoundsException e) {
            negative = "bounds";
        }

        return reversed + "," + negative;
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
