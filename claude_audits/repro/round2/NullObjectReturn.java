public class NullObjectReturn {
    static class Box { int v; Box(int v) { this.v = v; } }
    static Box find(int k) { return k > 0 ? new Box(k) : null; }
    public static String run() {
        Box a = find(2);
        Box b = find(-1);
        String s = (b == null) ? "none" : "" + b.v;
        return a.v + "," + s + "," + (a != null);
    }
    public static void main(String[] args) { System.out.println(run()); }
}
