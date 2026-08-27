import java.util.Map;

public class NullableFieldReceiverCatch {
    Holder holder;

    String firstKey() {
        return holder.getValues().entrySet().iterator().next().getKey();
    }

    static String run() {
        try {
            return new NullableFieldReceiverCatch().firstKey();
        } catch (NullPointerException e) {
            return "npe";
        }
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}

class Holder {
    Map<String, String> getValues() {
        return null;
    }
}
