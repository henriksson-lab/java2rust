import java.util.NoSuchElementException;
import java.util.TreeMap;

public class TreeMapFirstLastKeyCatch {
    public static String run() {
        TreeMap<String, Integer> values = new TreeMap<>();
        String firstStatus = "";
        try {
            values.firstKey();
            firstStatus = "miss";
        } catch (NoSuchElementException e) {
            firstStatus = "first";
        }
        values.put("b", 2);
        values.put("a", 1);
        String first = values.firstKey();
        String last = values.lastKey();
        TreeMap<String, Integer> empty = new TreeMap<>();
        String lastStatus = "";
        try {
            empty.lastKey();
            lastStatus = "miss";
        } catch (NoSuchElementException e) {
            lastStatus = "last";
        }
        return firstStatus + ":" + first + ":" + last + ":" + lastStatus;
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
