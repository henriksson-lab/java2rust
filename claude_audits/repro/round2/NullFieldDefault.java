public class NullFieldDefault {
    static class Holder {
        String name;
        int count;
    }
    public static String run() {
        Holder h = new Holder();
        boolean isNull = (h.name == null);
        String s = "v=" + h.name;
        h.name = "set";
        boolean after = (h.name == null);
        return isNull + "," + s + "," + after + "," + h.count;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
