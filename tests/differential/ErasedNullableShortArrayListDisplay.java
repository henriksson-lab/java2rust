import java.util.ArrayList;

public class ErasedNullableShortArrayListDisplay {
    public static String run() {
        ArrayList<Short> values = new ArrayList<Short>();
        values.add((short) 7);
        values.add(null);
        Object value = values;
        return "value=" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
