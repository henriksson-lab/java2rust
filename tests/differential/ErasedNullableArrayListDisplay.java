import java.util.ArrayList;

public class ErasedNullableArrayListDisplay {
    public static String run() {
        ArrayList<String> values = new ArrayList<String>();
        values.add("a");
        values.add(null);
        Object value = values;
        return "x" + value + ":" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
