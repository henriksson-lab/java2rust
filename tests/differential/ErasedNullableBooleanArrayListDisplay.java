import java.util.ArrayList;

public class ErasedNullableBooleanArrayListDisplay {
    public static String run() {
        ArrayList<Boolean> values = new ArrayList<Boolean>();
        values.add(true);
        values.add(null);
        Object value = values;
        return "value=" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
