public class ErasedClassDisplay {
    public static String run() {
        Object value = String.class;
        return "value=" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
