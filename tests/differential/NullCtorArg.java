public class NullCtorArg {
    static class Holder {
        String name;
        Holder(String n) { this.name = n; }
    }
    static String maybe(boolean b) { return b ? "x" : null; }
    public static String run() {
        Holder h = new Holder(maybe(false));
        Holder g = new Holder(maybe(true));
        return (h.name == null) + "," + h.name + "," + (g.name == null) + "," + g.name;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
