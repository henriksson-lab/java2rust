public class ErasedBoxedPrimitiveDisplay {
    public static String run() {
        Object i = 7;
        Object c = 'A';
        return "i=" + i + ",c=" + c + ",direct=" + i.toString() + "/" + c.toString();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
