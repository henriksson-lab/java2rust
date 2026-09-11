import java.util.ArrayList;

public class ErasedNullableCharacterArrayListDisplay {
    public static String run() {
        ArrayList<Character> values = new ArrayList<Character>();
        values.add('A');
        values.add(null);
        Object value = values;
        return "value=" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
