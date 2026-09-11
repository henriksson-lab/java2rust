import java.util.ArrayList;

public class ErasedNullableIntegerArrayListDisplay {
    public static String run() {
        ArrayList<Integer> values = new ArrayList<Integer>();
        values.add(7);
        values.add(null);
        Object value = values;
        return "value=" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
