import java.util.ArrayList;
import java.util.List;

public class ListElementReceiverNullCatch {
    public static String run() {
        List<String> values = new ArrayList<String>();
        values.add(null);
        try {
            return values.get(0).length() + "";
        } catch (NullPointerException e) {
            return "npe";
        }
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
