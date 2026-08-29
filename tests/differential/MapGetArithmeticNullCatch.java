import java.util.HashMap;

public class MapGetArithmeticNullCatch {
    public static void main(String[] args) {
        System.out.print(run());
    }

    public static String run() {
        HashMap<String, Integer> totals = new HashMap<String, Integer>();
        try {
            return "value:" + (totals.get("x") / 2);
        } catch (NullPointerException e) {
            return "npe";
        }
    }
}
