public class ErasedPlatformDisplay {
    public static String run() {
        Object s = "x";
        Object b = Boolean.TRUE;
        return "s=" + s + ",b=" + b + ",direct=" + s.toString() + "/" + b.toString();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
